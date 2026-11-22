import React from 'react';

interface SparkChartProps {
  title: string;
  unit: string;
  data: { timestamp: number; value: number }[];
  color?: string;
  maxOverride?: number;
}

export const SparkChart: React.FC<SparkChartProps> = ({
  title,
  unit,
  data,
  color = '#f59e0b',
  maxOverride,
}) => {
  const width = 560;
  const height = 150;
  const padX = 8;
  const padY = 12;

  const values = data.map((d) => d.value);
  const latest = values.length ? values[values.length - 1] : 0;
  const peak = values.length ? Math.max(...values) : 0;
  const maxVal = maxOverride ?? Math.max(peak * 1.15, 1);

  const points = data.map((d, idx) => {
    const x =
      data.length > 1
        ? padX + (idx / (data.length - 1)) * (width - padX * 2)
        : width / 2;
    const y =
      height - padY - (Math.min(d.value, maxVal) / maxVal) * (height - padY * 2);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const linePath = points.length > 1 ? `M ${points.join(' L ')}` : '';
  const areaPath =
    points.length > 1
      ? `${linePath} L ${width - padX},${height - padY} L ${padX},${height - padY} Z`
      : '';

  const gradId = `grad-${title.replace(/[^a-zA-Z0-9]/g, '')}`;

  return (
    <div className="card">
      <div className="card-header" style={{ marginBottom: 8 }}>
        <span className="card-title">{title}</span>
        <div style={{ display: 'flex', gap: 14, fontFamily: 'var(--font-mono)', fontSize: 12 }}>
          <span style={{ color: 'var(--text-secondary)' }}>
            Picco: <strong style={{ color: 'var(--text-primary)' }}>{peak.toFixed(1)} {unit}</strong>
          </span>
          <span style={{ color }}>
            Attuale: <strong>{latest.toFixed(1)} {unit}</strong>
          </span>
        </div>
      </div>

      {data.length < 2 ? (
        <div
          style={{
            height: 140,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'var(--text-muted)',
            fontSize: 12.5,
          }}
        >
          Raccolta campioni telemetria in corso (campionamento ogni 5s)...
        </div>
      ) : (
        <svg viewBox={`0 0 ${width} ${height}`} style={{ width: '100%', height: 145, display: 'block' }}>
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity="0.32" />
              <stop offset="100%" stopColor={color} stopOpacity="0.02" />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75].map((ratio) => (
            <line
              key={ratio}
              x1={padX}
              x2={width - padX}
              y1={padY + ratio * (height - padY * 2)}
              y2={padY + ratio * (height - padY * 2)}
              stroke="var(--border-subtle)"
              strokeDasharray="3 4"
            />
          ))}
          <path d={areaPath} fill={`url(#${gradId})`} />
          <path d={linePath} fill="none" stroke={color} strokeWidth="2.2" strokeLinejoin="round" />
        </svg>
      )}
    </div>
  );
};
