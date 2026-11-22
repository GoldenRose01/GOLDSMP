import React from 'react';

interface GaugeProps {
  label: string;
  value: number; // 0 to 100
  displayValue: string;
  sublabel?: string;
  color?: string;
}

export const Gauge: React.FC<GaugeProps> = ({
  label,
  value,
  displayValue,
  sublabel,
  color = 'var(--accent)',
}) => {
  const clamped = Math.max(0, Math.min(100, isNaN(value) ? 0 : value));
  const radius = 44;
  const circumference = 2 * Math.PI * radius;
  const arcLength = circumference * 0.75; // 270-degree AMP-style arc
  const dashOffset = arcLength - (clamped / 100) * arcLength;

  const strokeColor =
    clamped > 88 ? 'var(--danger)' : clamped > 70 ? 'var(--warning)' : color;

  return (
    <div
      className="card"
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '16px 12px',
        gap: 6,
      }}
    >
      <div style={{ position: 'relative', width: 112, height: 112 }}>
        <svg viewBox="0 0 110 110" width="112" height="112">
          <circle
            cx="55"
            cy="55"
            r={radius}
            fill="none"
            stroke="var(--bg-input)"
            strokeWidth="9"
            strokeLinecap="round"
            strokeDasharray={`${arcLength} ${circumference}`}
            transform="rotate(135 55 55)"
          />
          <circle
            cx="55"
            cy="55"
            r={radius}
            fill="none"
            stroke={strokeColor}
            strokeWidth="9"
            strokeLinecap="round"
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeDashoffset={dashOffset}
            transform="rotate(135 55 55)"
            style={{ transition: 'stroke-dashoffset 0.4s ease, stroke 0.3s ease' }}
          />
        </svg>
        <div
          style={{
            position: 'absolute',
            inset: 0,
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            paddingTop: 6,
          }}
        >
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontWeight: 700,
              fontSize: 16,
              color: 'var(--text-primary)',
            }}
          >
            {displayValue}
          </span>
          {sublabel && (
            <span style={{ fontSize: 10.5, color: 'var(--text-muted)', marginTop: 2 }}>
              {sublabel}
            </span>
          )}
        </div>
      </div>
      <span style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--text-secondary)' }}>
        {label}
      </span>
    </div>
  );
};
