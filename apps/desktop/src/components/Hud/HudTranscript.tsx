import React from "react";
import { ExecutionTelemetry, HudState } from "../../types/hud";

interface HudTranscriptProps {
  state: HudState;
  transcript: string;
  response: string;
  telemetry?: ExecutionTelemetry;
  onApprove?: () => void;
  onDeny?: () => void;
}

/**
 * Minimalist Futuristic HUD Transcript & Response Overlay
 * Replaces conventional chat windows with an elegant holographic readout.
 */
export const HudTranscript: React.FC<HudTranscriptProps> = ({
  state,
  transcript,
  response,
  telemetry,
  onApprove,
  onDeny,
}) => {
  if (state === "IDLE" && !transcript && !response) {
    return (
      <div className="hud-transcript-container idle-hint">
        <div className="hud-hint-text">AWAITING VOICE COMMAND OR INPUT</div>
      </div>
    );
  }

  return (
    <div className="hud-transcript-container">
      {/* ── Recognized User Speech ──────────────────────── */}
      {transcript && (
        <div className="hud-user-transcript">
          <span className="transcript-label">INPUT // </span>
          <span className="transcript-text">"{transcript}"</span>
        </div>
      )}

      {/* ── State / Action Telemetry Ticker ─────────────── */}
      {telemetry && (
        <div className="hud-telemetry-badge">
          {telemetry.toolName && (
            <span className="telemetry-item">
              TOOL: <span className="telemetry-val">{telemetry.toolName}</span>
            </span>
          )}
          {telemetry.applicationName && (
            <span className="telemetry-item">
              TARGET: <span className="telemetry-val">{telemetry.applicationName}</span>
            </span>
          )}
          {telemetry.processId && (
            <span className="telemetry-item">
              PID: <span className="telemetry-val">{telemetry.processId}</span>
            </span>
          )}
          {telemetry.durationMs !== undefined && (
            <span className="telemetry-item">
              TIME: <span className="telemetry-val">{telemetry.durationMs}ms</span>
            </span>
          )}
        </div>
      )}

      {/* ── J.A.R.V.I.S. Spoken Output ──────────────────── */}
      {response && (
        <div className={`hud-jarvis-response ${state === "ERROR" ? "error-text" : ""}`}>
          <span className="response-prefix">J.A.R.V.I.S. // </span>
          <span className="response-text">{response}</span>
        </div>
      )}

      {/* ── Human Authorization Prompt (if Awaiting) ───── */}
      {state === "AWAITING_USER" && (
        <div className="hud-approval-prompt">
          <div className="prompt-label">SECURITY POLICY CONFIRMATION REQUIRED</div>
          <div className="prompt-actions">
            <button className="hud-btn hud-btn-allow" onClick={onApprove}>
              AUTHORIZE
            </button>
            <button className="hud-btn hud-btn-deny" onClick={onDeny}>
              ABORT
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
