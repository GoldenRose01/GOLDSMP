use crate::db::Database;
use crate::files::resolve_safe_path;
use crate::process_manager::ProcessManager;
use rusqlite::params;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

pub fn ftp_username_for(instance_id: &str) -> String {
    let short = &instance_id[..instance_id.len().min(8)];
    format!("inst_{}", short)
}

pub fn get_or_create_ftp_password(db: &Database, instance_id: &str) -> String {
    let key = format!("ftp_pass_{}", instance_id);
    {
        let conn = db.conn();
        if let Ok(pass) = conn.query_row(
            "SELECT value FROM kv_settings WHERE key = ?1",
            params![key],
            |r| r.get::<_, String>(0),
        ) {
            return pass;
        }
    }
    rotate_ftp_password(db, instance_id)
}

pub fn rotate_ftp_password(db: &Database, instance_id: &str) -> String {
    let key = format!("ftp_pass_{}", instance_id);
    let pass = Uuid::new_v4().to_string().replace('-', "")[..16].to_string();
    let conn = db.conn();
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv_settings (key, value) VALUES (?1, ?2)",
        params![key, pass],
    );
    pass
}

pub fn spawn_ftp_server(db: Database, pm: Arc<ProcessManager>, ftp_port: u16) {
    tokio::spawn(async move {
        let addr = SocketAddr::from(([0, 0, 0, 0], ftp_port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => {
                tracing::info!("GOLDSMP FTP Server nativo attivo su ftp://0.0.0.0:{}", ftp_port);
                l
            }
            Err(e) => {
                tracing::error!("Impossibile avviare FTP server sulla porta {}: {}", ftp_port, e);
                return;
            }
        };

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let db_c = db.clone();
                    let pm_c = pm.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_ftp_client(stream, peer, db_c, pm_c).await {
                            tracing::debug!("Sessione FTP {} terminata: {}", peer, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("Errore accept FTP: {}", e);
                }
            }
        }
    });
}

async fn send_reply(writer: &mut tokio::net::tcp::OwnedWriteHalf, code: u16, msg: &str) -> std::io::Result<()> {
    writer
        .write_all(format!("{} {}\r\n", code, msg).as_bytes())
        .await?;
    writer.flush().await
}

async fn handle_ftp_client(
    stream: TcpStream,
    peer: SocketAddr,
    db: Database,
    pm: Arc<ProcessManager>,
) -> std::io::Result<()> {
    let local_ip = stream
        .local_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());
    let (read_half, mut writer) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    send_reply(&mut writer, 220, "GOLDSMP Native Rust FTP Server Ready").await?;

    let mut pending_user: Option<String> = None;
    let mut root_dir: Option<PathBuf> = None;
    let mut instance_name = String::new();
    let mut cwd_rel = String::new(); // relative to root_dir, no leading slash
    let mut pasv_listener: Option<TcpListener> = None;
    let mut rename_from: Option<PathBuf> = None;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("").to_uppercase();
        let arg = parts.next().unwrap_or("").trim();

        match cmd.as_str() {
            "USER" => {
                pending_user = Some(arg.to_string());
                send_reply(&mut writer, 331, "Username OK, password richiesta").await?;
            }
            "PASS" => {
                let user = match &pending_user {
                    Some(u) => u.clone(),
                    None => {
                        send_reply(&mut writer, 503, "Invia prima USER").await?;
                        continue;
                    }
                };
                // Match against any instance by inst_<short_id> or full id
                let instances = pm.list_instances();
                let matched = instances.into_iter().find(|inst| {
                    let expected_user = ftp_username_for(&inst.id);
                    (user == expected_user || user == inst.id)
                        && get_or_create_ftp_password(&db, &inst.id) == arg
                });

                if let Some(inst) = matched {
                    let wd = PathBuf::from(&inst.work_dir);
                    let _ = std::fs::create_dir_all(&wd);
                    root_dir = Some(wd);
                    instance_name = inst.name.clone();
                    cwd_rel.clear();
                    db.log_audit(
                        &user,
                        "FTP_LOGIN",
                        &inst.name,
                        &format!("Connessione FTP da {}", peer.ip()),
                        &peer.ip().to_string(),
                    );
                    send_reply(&mut writer, 230, "Accesso FTP riuscito").await?;
                } else {
                    send_reply(&mut writer, 530, "Credenziali FTP non valide").await?;
                }
            }
            "SYST" => send_reply(&mut writer, 215, "UNIX Type: L8").await?,
            "FEAT" => {
                writer
                    .write_all(b"211-Features:\r\n PASV\r\n SIZE\r\n UTF8\r\n211 End\r\n")
                    .await?;
                writer.flush().await?;
            }
            "OPTS" => send_reply(&mut writer, 200, "OK").await?,
            "TYPE" => send_reply(&mut writer, 200, "Type set").await?,
            "NOOP" => send_reply(&mut writer, 200, "OK").await?,
            "QUIT" => {
                send_reply(&mut writer, 221, "Arrivederci da GOLDSMP FTP").await?;
                break;
            }
            _ => {
                let root = match &root_dir {
                    Some(r) => r.clone(),
                    None => {
                        send_reply(&mut writer, 530, "Effettua prima il login").await?;
                        continue;
                    }
                };

                match cmd.as_str() {
                    "PWD" | "XPWD" => {
                        let shown = if cwd_rel.is_empty() {
                            "/".to_string()
                        } else {
                            format!("/{}", cwd_rel)
                        };
                        send_reply(&mut writer, 257, &format!("\"{}\" is current directory", shown)).await?;
                    }
                    "CWD" => {
                        let next_rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &next_rel) {
                            Ok(p) if p.is_dir() => {
                                cwd_rel = p
                                    .strip_prefix(&root)
                                    .unwrap_or(Path::new(""))
                                    .to_string_lossy()
                                    .replace('\\', "/");
                                send_reply(&mut writer, 250, "Directory cambiata").await?;
                            }
                            _ => send_reply(&mut writer, 550, "Directory non trovata").await?,
                        }
                    }
                    "CDUP" => {
                        let mut parts: Vec<&str> = cwd_rel.split('/').filter(|s| !s.is_empty()).collect();
                        parts.pop();
                        cwd_rel = parts.join("/");
                        send_reply(&mut writer, 250, "Directory superiore").await?;
                    }
                    "PASV" => {
                        match TcpListener::bind("0.0.0.0:0").await {
                            Ok(dl) => {
                                let port = dl.local_addr()?.port();
                                let p1 = port / 256;
                                let p2 = port % 256;
                                let ip_str = std::env::var("GOLDSMP_FTP_PUBLIC_IP")
                                    .unwrap_or_else(|_| local_ip.clone());
                                let ip_commas = if ip_str.contains('.') {
                                    ip_str.replace('.', ",")
                                } else {
                                    "127,0,0,1".into()
                                };
                                pasv_listener = Some(dl);
                                send_reply(
                                    &mut writer,
                                    227,
                                    &format!("Entering Passive Mode ({},{},{})", ip_commas, p1, p2),
                                )
                                .await?;
                            }
                            Err(_) => send_reply(&mut writer, 425, "Impossibile aprire porta passiva").await?,
                        }
                    }
                    "LIST" | "NLST" => {
                        let dl = match pasv_listener.take() {
                            Some(l) => l,
                            None => {
                                send_reply(&mut writer, 425, "Usa PASV prima di LIST").await?;
                                continue;
                            }
                        };
                        let clean_arg = arg
                            .split_whitespace()
                            .filter(|s| !s.starts_with('-'))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let target_rel = if clean_arg.is_empty() {
                            cwd_rel.clone()
                        } else {
                            combine_rel(&cwd_rel, &clean_arg)
                        };
                        let target = match resolve_safe_path(&root, &target_rel) {
                            Ok(p) => p,
                            Err(_) => {
                                send_reply(&mut writer, 550, "Percorso non valido").await?;
                                continue;
                            }
                        };
                        send_reply(&mut writer, 150, "Invio elenco file").await?;
                        if let Ok((mut data_stream, _)) = dl.accept().await {
                            let mut listing = String::new();
                            if let Ok(rd) = std::fs::read_dir(&target) {
                                for entry in rd.flatten() {
                                    if let Ok(meta) = entry.metadata() {
                                        let name = entry.file_name().to_string_lossy().to_string();
                                        if cmd == "NLST" {
                                            listing.push_str(&format!("{}\r\n", name));
                                        } else {
                                            let perms = if meta.is_dir() { "drwxr-xr-x" } else { "-rw-r--r--" };
                                            let size = if meta.is_dir() { 0 } else { meta.len() };
                                            listing.push_str(&format!(
                                                "{} 1 goldsmp goldsmp {:>10} Jan 01 00:00 {}\r\n",
                                                perms, size, name
                                            ));
                                        }
                                    }
                                }
                            }
                            let _ = data_stream.write_all(listing.as_bytes()).await;
                            let _ = data_stream.shutdown().await;
                        }
                        send_reply(&mut writer, 226, "Elenco completato").await?;
                    }
                    "SIZE" => {
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel).and_then(|p| std::fs::metadata(p).map_err(|e| e.to_string())) {
                            Ok(m) => send_reply(&mut writer, 213, &m.len().to_string()).await?,
                            Err(_) => send_reply(&mut writer, 550, "File non trovato").await?,
                        }
                    }
                    "RETR" => {
                        let dl = match pasv_listener.take() {
                            Some(l) => l,
                            None => {
                                send_reply(&mut writer, 425, "Usa PASV prima di RETR").await?;
                                continue;
                            }
                        };
                        let rel = combine_rel(&cwd_rel, arg);
                        let target = match resolve_safe_path(&root, &rel) {
                            Ok(p) if p.is_file() => p,
                            _ => {
                                send_reply(&mut writer, 550, "File non trovato").await?;
                                continue;
                            }
                        };
                        send_reply(&mut writer, 150, "Avvio download file").await?;
                        if let Ok((mut data_stream, _)) = dl.accept().await {
                            if let Ok(mut file) = tokio::fs::File::open(&target).await {
                                let _ = tokio::io::copy(&mut file, &mut data_stream).await;
                            }
                            let _ = data_stream.shutdown().await;
                        }
                        send_reply(&mut writer, 226, "Download completato").await?;
                    }
                    "STOR" => {
                        let dl = match pasv_listener.take() {
                            Some(l) => l,
                            None => {
                                send_reply(&mut writer, 425, "Usa PASV prima di STOR").await?;
                                continue;
                            }
                        };
                        let rel = combine_rel(&cwd_rel, arg);
                        let target = match resolve_safe_path(&root, &rel) {
                            Ok(p) => p,
                            Err(_) => {
                                send_reply(&mut writer, 550, "Percorso non consentito").await?;
                                continue;
                            }
                        };
                        if let Some(parent) = target.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        send_reply(&mut writer, 150, "Pronto a ricevere file").await?;
                        if let Ok((mut data_stream, _)) = dl.accept().await {
                            if let Ok(mut file) = tokio::fs::File::create(&target).await {
                                let mut buf = vec![0u8; 32768];
                                loop {
                                    match data_stream.read(&mut buf).await {
                                        Ok(0) | Err(_) => break,
                                        Ok(n) => {
                                            if file.write_all(&buf[..n]).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        db.log_audit(
                            pending_user.as_deref().unwrap_or("ftp"),
                            "FTP_UPLOAD",
                            &instance_name,
                            &format!("Caricato {}", rel),
                            &peer.ip().to_string(),
                        );
                        send_reply(&mut writer, 226, "Upload completato").await?;
                    }
                    "DELE" => {
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel).and_then(|p| std::fs::remove_file(p).map_err(|e| e.to_string())) {
                            Ok(_) => send_reply(&mut writer, 250, "File eliminato").await?,
                            Err(_) => send_reply(&mut writer, 550, "Impossibile eliminare il file").await?,
                        }
                    }
                    "MKD" | "XMKD" => {
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel).and_then(|p| std::fs::create_dir_all(p).map_err(|e| e.to_string())) {
                            Ok(_) => send_reply(&mut writer, 257, "Cartella creata").await?,
                            Err(_) => send_reply(&mut writer, 550, "Impossibile creare cartella").await?,
                        }
                    }
                    "RMD" | "XRMD" => {
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel).and_then(|p| std::fs::remove_dir_all(p).map_err(|e| e.to_string())) {
                            Ok(_) => send_reply(&mut writer, 250, "Cartella rimossa").await?,
                            Err(_) => send_reply(&mut writer, 550, "Impossibile rimuovere cartella").await?,
                        }
                    }
                    "RNFR" => {
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel) {
                            Ok(p) if p.exists() => {
                                rename_from = Some(p);
                                send_reply(&mut writer, 350, "Pronto per RNTO").await?;
                            }
                            _ => send_reply(&mut writer, 550, "Percorso sorgente non trovato").await?,
                        }
                    }
                    "RNTO" => {
                        let src = match rename_from.take() {
                            Some(s) => s,
                            None => {
                                send_reply(&mut writer, 503, "Invia prima RNFR").await?;
                                continue;
                            }
                        };
                        let rel = combine_rel(&cwd_rel, arg);
                        match resolve_safe_path(&root, &rel).and_then(|dst| std::fs::rename(src, dst).map_err(|e| e.to_string())) {
                            Ok(_) => send_reply(&mut writer, 250, "Rinominato con successo").await?,
                            Err(_) => send_reply(&mut writer, 550, "Errore rinomina").await?,
                        }
                    }
                    _ => {
                        send_reply(&mut writer, 502, "Comando non supportato").await?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn combine_rel(cwd: &str, arg: &str) -> String {
    if arg.starts_with('/') {
        arg.trim_start_matches('/').to_string()
    } else if cwd.is_empty() {
        arg.to_string()
    } else {
        format!("{}/{}", cwd, arg)
    }
}
