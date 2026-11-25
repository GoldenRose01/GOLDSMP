import React, { useEffect, useRef, useState } from 'react';

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
  color = '#f5b932',
  maxOverride,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [chartWidth, setChartWidth] = useState(520);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const updateSize = () => {
      const w = el.clientWidth;
      if (w > 100) {
        setChartWidth(Math.floor(w));
      }
    };
    updateSize();
    if (typeof ResizeObserver !== 'undefined') {
      const ro = new ResizeObserver(updateSize);
      ro.observe(el);
      return () => ro.disconnect();
    } else {
      window.addEventListener('resize', updateSize);
      return () => window.removeEventListener('resize', updateSize);
    }
  }, []);

  const width = Math.max(220, chartWidth);
  const height = width < 360 ? 128 : 148;
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
    <div className="card" ref={containerRef} style={{ minWidth: 0, overflow: 'hidden' }}>
      <div className="card-header" style={{ marginBottom: 8, flexWrap: 'wrap', gap: 6 }}>
        <span className="card-title" style={{ fontSize: 14.5 }}>{title}</span>
        <div
          style={{
            display: 'flex',
            gap: 12,
            flexWrap: 'wrap',
            fontFamily: 'var(--font-mono)',
            fontSize: 11.5,
          }}
        >
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
            height: height - 10,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            textAlign: 'center',
            color: 'var(--text-muted)',
            fontSize: 12,
            padding: '0 8px',
          }}
        >
          Raccolta campioni telemetria in corso (ogni 5s)...
        </div>
      ) : (
        <svg
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          style={{ width: '100%', height, display: 'block', overflow: 'visible' }}
        >
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity="0.30" />
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
          <path
            d={linePath}
            fill="none"
            stroke={color}
            strokeWidth="2"
            strokeLinejoin="round"
            strokeLinecap="round"
            vectorEffect="non-scaling-stroke"
          />
        </svg>
      )}
    </div>
  );
};

