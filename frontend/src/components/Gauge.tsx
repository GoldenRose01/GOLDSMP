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
  const arcLength = circumference * 0.75; // 270-degree arc
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
        padding: '14px 10px',
        gap: 6,
        minWidth: 0,
      }}
    >
      <div
        style={{
          position: 'relative',
          width: '100%',
          maxWidth: 112,
          aspectRatio: '1 / 1',
          margin: '0 auto',
        }}
      >
        <svg
          viewBox="0 0 116 116"
          style={{ width: '100%', height: '100%', display: 'block' }}
        >
          {/* Outer segmented ring */}
          <circle
            cx="58"
            cy="58"
            r="53"
            fill="none"
            stroke="rgba(245, 185, 50, 0.16)"
            strokeWidth="1.2"
            strokeDasharray="6 4"
          />
          {/* Inner ring */}
          <circle
            cx="58"
            cy="58"
            r="35"
            fill="none"
            stroke="rgba(56, 189, 248, 0.12)"
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
          {/* Active arc */}
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
              transition: 'stroke-dashoffset 0.35s ease, stroke 0.25s ease',
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
            paddingTop: 4,
            textAlign: 'center',
          }}
        >
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontWeight: 700,
              fontSize: 'clamp(12px, 1.3vw, 15px)',
              color: '#fff6dc',
              lineHeight: 1.15,
            }}
          >
            {displayValue}
          </span>
          {sublabel && (
            <span
              style={{
                fontSize: 10,
                color: 'var(--text-muted)',
                marginTop: 2,
                maxWidth: '90%',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {sublabel}
            </span>
          )}
        </div>
      </div>
      <span
        style={{
          fontFamily: 'var(--font-display)',
          fontSize: 12.5,
          fontWeight: 700,
          letterSpacing: '0.06em',
          textTransform: 'uppercase',
          color: 'var(--text-secondary)',
          textAlign: 'center',
        }}
      >
        {label}
      </span>
    </div>
  );
};

