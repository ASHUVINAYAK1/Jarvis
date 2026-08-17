import React from "react";
import { HudState } from "../../types/hud";

interface HudSvgProps {
  state: HudState;
  audioLevel: number; // 0.0 to 1.0
}

/**
 * Concentric Holographic HUD Rings & Segments
 * Faithfully matches the iconic circular J.A.R.V.I.S. interface reference:
 * - Outer segmented notched ring with tick marks and data markers
 * - Secondary rotating ring track with yellow and cyan pips
 * - Tertiary arc segments with directional chevron markers
 * - Center glowing core ring
 */
export const HudSvg: React.FC<HudSvgProps> = ({ state, audioLevel }) => {
  // Speed multiplier based on state
  const isFast = state === "PROCESSING" || state === "PLANNING" || state === "EXECUTING";
  const isExcited = state === "WAKE_DETECTED" || state === "SPEAKING" || state === "LISTENING";
  const isSuccess = state === "SUCCESS";
  const isError = state === "ERROR";

  // Dynamic glow color
  const glowColor = isError
    ? "rgba(255, 60, 90, 0.85)"
    : isSuccess
    ? "rgba(0, 255, 180, 0.9)"
    : state === "AWAITING_USER"
    ? "rgba(255, 170, 40, 0.85)"
    : "rgba(0, 220, 255, 0.85)";

  const strokeColor = isError
    ? "#ff3c5a"
    : isSuccess
    ? "#00ffb4"
    : state === "AWAITING_USER"
    ? "#ffaa28"
    : "#00d4ff";

  // Generate outer tick marks (60 radial ticks)
  const ticks = Array.from({ length: 60 }, (_, i) => {
    const angle = (i * 360) / 60;
    const isMajor = i % 5 === 0;
    const r1 = isMajor ? 282 : 288;
    const r2 = 296;
    const rad = (angle * Math.PI) / 180;
    const x1 = 300 + r1 * Math.cos(rad);
    const y1 = 300 + r1 * Math.sin(rad);
    const x2 = 300 + r2 * Math.cos(rad);
    const y2 = 300 + r2 * Math.sin(rad);
    return (
      <line
        key={i}
        x1={x1}
        y1={y1}
        x2={x2}
        y2={y2}
        stroke={strokeColor}
        strokeWidth={isMajor ? 2 : 1}
        opacity={isMajor ? 0.75 : 0.35}
      />
    );
  });

  // Generate secondary inner dot tracks
  const innerDots = Array.from({ length: 24 }, (_, i) => {
    const angle = (i * 360) / 24;
    const rad = (angle * Math.PI) / 180;
    const r = 185;
    const cx = 300 + r * Math.cos(rad);
    const cy = 300 + r * Math.sin(rad);
    return (
      <circle
        key={i}
        cx={cx}
        cy={cy}
        r={1.8}
        fill={strokeColor}
        opacity={0.5}
      />
    );
  });

  return (
    <div className={`hud-svg-container ${state.toLowerCase()}`}>
      <svg
        viewBox="0 0 600 600"
        className="hud-svg"
        style={{
          filter: `drop-shadow(0 0 ${12 + audioLevel * 18}px ${glowColor})`,
        }}
      >
        <defs>
          <filter id="glow" x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation="3.5" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>

          <linearGradient id="cyanBlueGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#00d4ff" />
            <stop offset="60%" stopColor="#0088ff" />
            <stop offset="100%" stopColor="#0044cc" />
          </linearGradient>

          <linearGradient id="accentYellowGrad" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#ffb400" />
            <stop offset="100%" stopColor="#ff8800" />
          </linearGradient>
        </defs>

        {/* ── Layer 1: Outermost Thin Guide Ring ────────────── */}
        <circle
          cx="300"
          cy="300"
          r="298"
          fill="none"
          stroke={strokeColor}
          strokeWidth="0.75"
          opacity="0.25"
          strokeDasharray="4 8"
        />

        {/* ── Layer 2: Radial Tick Ring ──────────────────────── */}
        <g className={`ring-layer ring-ticks ${isFast ? "fast-cw" : "slow-cw"}`}>
          {ticks}
        </g>

        {/* ── Layer 3: Main Outer Segmented Arc Ring (Cyan/Blue) ── */}
        <g className={`ring-layer ring-outer-segments ${isFast ? "fast-ccw" : "slow-ccw"}`}>
          {/* Top-Right Major Segment Arc */}
          <path
            d="M 490 190 A 240 240 0 0 1 540 300"
            fill="none"
            stroke="url(#cyanBlueGrad)"
            strokeWidth="14"
            strokeLinecap="round"
            opacity="0.9"
          />
          {/* Top-Left Major Segment Arc */}
          <path
            d="M 110 190 A 240 240 0 0 1 300 60"
            fill="none"
            stroke="url(#cyanBlueGrad)"
            strokeWidth="14"
            strokeLinecap="round"
            opacity="0.85"
          />
          {/* Bottom Arc Segment */}
          <path
            d="M 160 440 A 240 240 0 0 0 440 440"
            fill="none"
            stroke="url(#cyanBlueGrad)"
            strokeWidth="8"
            strokeLinecap="round"
            opacity="0.7"
          />
          {/* Notched outer brackets */}
          <path
            d="M 300 45 L 310 45 L 320 55 L 280 55 L 290 45 Z"
            fill={strokeColor}
            opacity="0.8"
          />
          <path
            d="M 300 555 L 310 555 L 320 545 L 280 545 L 290 555 Z"
            fill={strokeColor}
            opacity="0.8"
          />
        </g>

        {/* ── Layer 4: Yellow/Orange Indicator Arc (Reference feature) ── */}
        <g className={`ring-layer ring-accent ${isFast ? "fast-cw" : "slow-cw"}`}>
          <path
            d="M 140 220 A 210 210 0 0 1 180 130"
            fill="none"
            stroke="url(#accentYellowGrad)"
            strokeWidth="7"
            strokeLinecap="round"
            opacity={isError ? "0.2" : "0.95"}
          />
          {/* Yellow indicator pips along arc */}
          <circle cx="215" cy="105" r="3.5" fill="#ffcc00" opacity="0.9" />
          <circle cx="255" cy="92" r="3.5" fill="#ffcc00" opacity="0.9" />
          <circle cx="345" cy="92" r="3.5" fill="#ffcc00" opacity="0.9" />
        </g>

        {/* ── Layer 5: Concentric Segmented Dash Ring ───────── */}
        <g className={`ring-layer ring-mid-dashes ${isFast ? "fast-cw" : "med-cw"}`}>
          <circle
            cx="300"
            cy="300"
            r="165"
            fill="none"
            stroke={strokeColor}
            strokeWidth="2.5"
            strokeDasharray="18 12 6 12 36 12"
            opacity="0.75"
          />
        </g>

        {/* ── Layer 6: Inner Dot Track ──────────────────────── */}
        <g className={`ring-layer ring-dots ${isFast ? "fast-ccw" : "slow-ccw"}`}>
          {innerDots}
        </g>

        {/* ── Layer 7: Center Inner Reticle Ring (Reacts to Audio) ── */}
        <g className="ring-layer ring-inner-core">
          <circle
            cx="300"
            cy="300"
            r={115 + audioLevel * 14}
            fill="none"
            stroke={strokeColor}
            strokeWidth={2 + audioLevel * 2}
            strokeDasharray={isExcited ? "40 10 20 10" : "80 20"}
            opacity={0.85}
            style={{
              transition: "r 0.08s ease, stroke-width 0.08s ease",
            }}
          />
          {/* Small Crosshairs */}
          <line x1="175" y1="300" x2="185" y2="300" stroke={strokeColor} strokeWidth="1.5" opacity="0.6" />
          <line x1="415" y1="300" x2="425" y2="300" stroke={strokeColor} strokeWidth="1.5" opacity="0.6" />
          <line x1="300" y1="175" x2="300" y2="185" stroke={strokeColor} strokeWidth="1.5" opacity="0.6" />
          <line x1="300" y1="415" x2="300" y2="425" stroke={strokeColor} strokeWidth="1.5" opacity="0.6" />
        </g>

        {/* ── Layer 8: Innermost Glowing Boundary ────────────── */}
        <circle
          cx="300"
          cy="300"
          r={95 + audioLevel * 8}
          fill="none"
          stroke={strokeColor}
          strokeWidth="1"
          opacity="0.4"
          strokeDasharray="3 6"
        />
      </svg>
    </div>
  );
};
