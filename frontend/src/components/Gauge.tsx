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
  const arcLength = circumference * 0.75; // 270-degree Stark HUD arc
  const dashOffset = arcLength - (clamped / 100) * arcLength;

  const strokeColor =
    clamped > 88 ? 'var(--danger)' : clamped > 72 ? 'var(--warning)' : color;

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
      <div style={{ position: 'relative', width: 118, height: 118 }}>
        <svg viewBox="0 0 116 116" width="118" height="118">
          {/* Outer HUD segmented ring */}
          <circle
            cx="58"
            cy="58"
            r="53"
            fill="none"
            stroke="rgba(245, 185, 50, 0.18)"
            strokeWidth="1.2"
            strokeDasharray="6 4"
          />
          {/* Inner Arc Reactor ring */}
          <circle
            cx="58"
            cy="58"
            r="35"
            fill="none"
            stroke="rgba(56, 189, 248, 0.14)"
            strokeWidth="1"
          />
          {/* Track */}
          <circle
            cx="58"
            cy="58"
            r={radius}
            fill="none"
            stroke="rgba(255, 255, 255, 0.06)"
            strokeWidth="8.5"
            strokeLinecap="round"
            strokeDasharray={`${arcLength} ${circumference}`}
            transform="rotate(135 58 58)"
          />
          {/* Active Stark Gold / Arc bar */}
          <circle
            cx="58"
            cy="58"
            r={radius}
            fill="none"
            stroke={strokeColor}
            strokeWidth="8.5"
            strokeLinecap="round"
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeDashoffset={dashOffset}
            transform="rotate(135 58 58)"
            style={{
              transition: 'stroke-dashoffset 0.4s ease, stroke 0.3s ease',
              filter: 'drop-shadow(0 0 6px rgba(245, 185, 50, 0.45))',
            }}
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
              color: '#fff4d2',
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
      <span
        style={{
          fontFamily: 'var(--font-display)',
          fontSize: 13.5,
          fontWeight: 700,
          letterSpacing: '0.07em',
          textTransform: 'uppercase',
          color: 'var(--text-secondary)',
        }}
      >
        {label}
      </span>
    </div>
  );
};
