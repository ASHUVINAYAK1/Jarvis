import React from "react";
import { HudState } from "../../types/hud";

interface HudCoreProps {
  state: HudState;
  audioLevel: number;
}

/**
 * Centered J.A.R.V.I.S. Core Identity
 * - Renders wide-spaced futuristic typography
 * - Chromatic aberration and pulse glow
 * - Dynamic status sub-label (e.g. LISTENING, OPENING CHROME, READY)
 */
export const HudCore: React.FC<HudCoreProps> = ({ state, audioLevel }) => {
  const getStatusText = () => {
    switch (state) {
      case "IDLE":
        return "ONLINE // STANDBY";
      case "WAKE_DETECTED":
        return "SYSTEM ENGAGED";
      case "LISTENING":
        return "LISTENING...";
      case "PROCESSING":
        return "ANALYZING INTENT";
      case "PLANNING":
        return "SYNTHESIZING PLAN";
      case "EXECUTING":
        return "EXECUTING ACTION";
      case "AWAITING_USER":
        return "AWAITING AUTHORIZATION";
      case "SPEAKING":
        return "TRANSMITTING";
      case "SUCCESS":
        return "ACTION COMPLETE";
      case "ERROR":
        return "SYSTEM ALERT";
      default:
        return "READY";
    }
  };

  const isError = state === "ERROR";
  const isSuccess = state === "SUCCESS";
  const isAwaiting = state === "AWAITING_USER";

  const colorClass = isError
    ? "core-error"
    : isSuccess
    ? "core-success"
    : isAwaiting
    ? "core-warning"
    : "core-cyan";

  return (
    <div className={`hud-core ${colorClass}`}>
      <div
        className="hud-core-glow"
        style={{
          transform: `scale(${1 + audioLevel * 0.3})`,
          opacity: 0.5 + audioLevel * 0.5,
        }}
      />
      <div className="hud-title-container">
        <h1 className="hud-title">J.A.R.V.I.S.</h1>
      </div>
      <div className="hud-status-badge">
        <span className="status-indicator-dot" />
        <span className="status-text">{getStatusText()}</span>
      </div>
    </div>
  );
};
