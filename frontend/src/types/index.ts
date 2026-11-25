export interface User {
  id: string;
  username: string;
  email: string;
  role: string;
  permissions: string[];
  oidc_subject?: string;
  created_at: string;
  last_login?: string;
}

export interface TemplateSettingField {
  key: string;
  label: string;
  description: string;
  field_type: 'string' | 'number' | 'boolean' | 'select';
  default_value: string;
  options?: string[];
  category: string;
  config_file?: string;
}

export interface AppTemplate {
  id: string;
  name: string;
  category: 'horizon' | 'minecraft' | 'steamcmd' | 'webservice' | 'generic';
  description: string;
  icon: string;
  default_port: number;
  default_executable: string;
  default_args: string[];
  stop_command: string;
  steam_app_id?: number;
  supports_modrinth: boolean;
  mod_folder?: string;
  settings_schema: TemplateSettingField[];
}

export interface Instance {
  id: string;
  name: string;
  template_id: string;
  category: string;
  status: 'stopped' | 'starting' | 'running' | 'stopping' | 'crashed';
  port: number;
  work_dir: string;
  executable: string;
  args: string[];
  env_vars: Record<string, string>;
  settings: Record<string, string>;
  auto_start: boolean;
  auto_restart: boolean;
  max_memory_mb: number;
  max_players: number;
  pid?: number;
  cpu_percent: number;
  memory_mb: number;
  disk_mb: number;
  active_players: string[];
  uptime_secs: number;
  created_at: string;
}

export interface ConsoleLine {
  timestamp: string;
  stream: 'stdout' | 'stderr' | 'system' | 'stdin';
  message: string;
}

export interface MetricPoint {
  timestamp: number;
  instance_id: string;
  cpu_percent: number;
  memory_mb: number;
  disk_mb: number;
  net_rx_kb: number;
  net_tx_kb: number;
  players_online: number;
}

export interface PlayerSession {
  id: string;
  instance_id: string;
  instance_name: string;
  player_name: string;
  ip_address: string;
  country_code: string;
  country_name: string;
  joined_at: string;
  left_at?: string;
  duration_secs: number;
}

export interface PlayerLeaderboardEntry {
  player_name: string;
  total_sessions: number;
  total_playtime_secs: number;
  last_seen: string;
  country_code: string;
  country_name: string;
  favorite_instance: string;
}

export interface GeoStat {
  country_code: string;
  country_name: string;
  unique_players: number;
  total_sessions: number;
  total_playtime_secs: number;
}

export interface AnalyticsOverview {
  unique_players: number;
  total_sessions: number;
  active_sessions: number;
  total_playtime_secs: number;
  avg_session_secs: number;
  countries_count: number;
  leaderboard: PlayerLeaderboardEntry[];
  geo_stats: GeoStat[];
  recent_sessions: PlayerSession[];
}

export interface GeoFilterRule {
  id: string;
  instance_id: string;
  country_code: string;
  country_name: string;
  action: 'allow' | 'block';
  enabled: boolean;
  created_at: string;
}

export interface ScheduledTask {
  id: string;
  instance_id: string;
  name: string;
  trigger_type: 'interval' | 'event';
  interval_secs?: number;
  event_name?: string;
  action_type: 'command' | 'restart' | 'backup' | 'webhook';
  action_payload: string;
  enabled: boolean;
  last_run?: string;
  run_count: number;
  created_at: string;
}

export interface BackupEntry {
  id: string;
  instance_id: string;
  name: string;
  filename: string;
  size_bytes: number;
  created_at: string;
  notes: string;
}

export interface WebhookConfig {
  id: string;
  name: string;
  url: string;
  events: string[];
  enabled: boolean;
  secret: string;
  last_triggered?: string;
  last_status?: number;
  created_at: string;
}

export interface AuditLogEntry {
  id: number;
  timestamp: string;
  username: string;
  action: string;
  target: string;
  details: string;
  ip_address: string;
}

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: string;
}

export interface HostSystemInfo {
  hostname: string;
  os_name: string;
  os_version: string;
  kernel_version: string;
  cpu_brand: string;
  cpu_cores: number;
  cpu_usage_percent: number;
  total_memory_mb: number;
  used_memory_mb: number;
  total_disk_gb: number;
  used_disk_gb: number;
  uptime_secs: number;
  panel_memory_mb: number;
  panel_version: string;
}

export interface PanelSettings {
  panel_name: string;
  accent_color: string;
  logo_url: string;
  oidc_enabled: boolean;
  oidc_issuer_url: string;
  oidc_client_id: string;
  oidc_client_secret: string;
  metrics_retention_days: number;
  incoming_webhook_token: string;
}

export interface ModrinthHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  icon_url?: string;
  project_type: string;
  categories: string[];
  status?: string;
}

export interface ModrinthAuthStatus {
  connected: boolean;
  user_id: string;
  username: string;
  avatar_url?: string;
}

export interface UpdaterConfig {
  repo: string;
  auto_check: boolean;
  auto_download: boolean;
  check_interval_mins: number;
}

export interface UpdateStatus {
  current_version: string;
  latest_version?: string;
  active_ui_version: string;
  update_available: boolean;
  ui_hot_updated: boolean;
  binary_staged: boolean;
  state: 'idle' | 'checking' | 'downloading' | 'ready_refresh' | 'up_to_date' | 'error';
  release_notes: string;
  release_url: string;
  last_checked?: string;
  error?: string;
  config: UpdaterConfig;
}

export interface FtpInfo {
  host: string;
  port: number;
  username: string;
  password: string;
  root_dir: string;
  quick_folders: string[];
}

export interface InstalledModEntry {
  filename: string;
  folder: string;
  size_bytes: number;
  enabled: boolean;
  sha1: string;
  modrinth_project_id?: string;
  current_version?: string;
  latest_version?: string;
  latest_filename?: string;
  latest_url?: string;
  update_available: boolean;
}

export interface GDrivePublicStatus {
  enabled: boolean;
  client_id: string;
  has_client_secret: boolean;
  redirect_uri: string;
  server_folder_name: string;
  max_backups_per_instance: number;
  connected: boolean;
  connected_email?: string;
  auth_mode?: string;
  cli_available?: boolean;
}

export interface McLoaderStatus {
  loader: string;
  available_versions: string[];
  jar_exists: boolean;
  jar_size_bytes: number;
  active_loader: string;
  active_version: string;
}

export interface SentinelConfig {
  enabled: boolean;
  check_interval_secs: number;
  auto_heal_crashed: boolean;
  auto_heal_unresponsive_port: boolean;
  cpu_warn_threshold: number;
  mem_warn_threshold: number;
  disk_warn_threshold: number;
  push_endpoint: string;
  push_token: string;
  push_interval_secs: number;
}

export interface SentinelInstanceHealth {
  instance_id: string;
  instance_name: string;
  template_id: string;
  port: number;
  process_status: string;
  health_state: 'healthy' | 'starting' | 'unhealthy' | 'degraded' | 'stopped' | 'healing';
  port_reachable: boolean;
  latency_ms?: number;
  cpu_percent: number;
  memory_mb: number;
  max_memory_mb: number;
  memory_usage_pct: number;
  consecutive_failures: number;
  auto_heals_count: number;
  last_checked: string;
}

export interface SentinelEvent {
  timestamp: string;
  level: 'info' | 'warn' | 'critical' | 'heal';
  target: string;
  message: string;
}

export interface SentinelStatus {
  config: SentinelConfig;
  agent_state: 'active' | 'paused' | 'warning' | 'critical';
  last_heartbeat?: string;
  total_checks: number;
  total_auto_heals: number;
  active_warnings: string[];
  host_cpu_pct: number;
  host_mem_pct: number;
  host_disk_pct: number;
  instances: SentinelInstanceHealth[];
  recent_events: SentinelEvent[];
  last_push_status?: string;
}
