import React, { useState, useEffect } from "react";
import { ExecutionTelemetry, HudState } from "../../types/hud";

interface HudTranscriptProps {
  state: HudState;
  transcript: string;
  response: string;
  telemetry?: ExecutionTelemetry;
  onApprove?: () => void;
  onDeny?: () => void;
}

const invokeTauri = async (cmd: string, args: any) => {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke(cmd, args);
    } catch (err) {
      console.warn("Tauri invoke error:", err);
    }
  }
  return null;
};

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
  const [imgData, setImgData] = useState<string | null>(null);

  useEffect(() => {
    if (telemetry?.path && (telemetry.artifactType === "screenshot" || telemetry.toolName?.includes("screenshot"))) {
      invokeTauri("get_screenshot_base64", { path: telemetry.path }).then((data: any) => {
        if (data && typeof data === "string") {
          setImgData(data);
        }
      });
    } else {
      setImgData(null);
    }
  }, [telemetry]);

  const handleOpenScreenshot = () => {
    if (telemetry?.path) {
      invokeTauri("open_screenshot", { path: telemetry.path });
    }
  };

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

      {/* ── Screenshot Artifact Holographic Card ────── */}
      {telemetry?.path && (telemetry.artifactType === "screenshot" || telemetry.toolName?.includes("screenshot")) && (
        <div className="hud-artifact-card">
          <div className="artifact-header">
            <span className="artifact-tag">ARTIFACT // SCREENSHOT</span>
            {telemetry.filename && <span className="artifact-filename">{telemetry.filename}</span>}
            {telemetry.width && telemetry.height && (
              <span className="artifact-dims">{telemetry.width} × {telemetry.height}</span>
            )}
          </div>
          {imgData && (
            <div className="artifact-preview-box">
              <img src={imgData} alt="Screenshot Preview" className="artifact-img-preview" />
            </div>
          )}
          <div className="artifact-actions">
            <button className="hud-btn hud-btn-open" onClick={handleOpenScreenshot}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" style={{ marginRight: 6 }}>
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                <polyline points="15 3 21 3 21 9" />
                <line x1="10" y1="14" x2="21" y2="3" />
              </svg>
              OPEN IMAGE
            </button>
          </div>
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
