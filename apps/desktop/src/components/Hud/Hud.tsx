import React, { useState, useEffect, useCallback, useRef } from "react";
import { HudSvg } from "./HudSvg";
import { HudCore } from "./HudCore";
import { HudVisualizer } from "./HudVisualizer";
import { HudTranscript } from "./HudTranscript";
import { ExecutionTelemetry, HudState } from "../../types/hud";
import "./Hud.css";

// Dynamic Tauri invoke if running in desktop app
const invokeTauri = async (cmd: string, args: any) => {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      return await invoke(cmd, args);
    } catch (err) {
      console.warn("Tauri invoke error, falling back to simulated pipeline:", err);
    }
  }
  return null;
};

export const Hud: React.FC = () => {
  const [state, setState] = useState<HudState>("IDLE");
  const [transcript, setTranscript] = useState<string>("");
  const [response, setResponse] = useState<string>("");
  const [telemetry, setTelemetry] = useState<ExecutionTelemetry | undefined>();
  const [audioLevel, setAudioLevel] = useState<number>(0);
  const [inputText, setInputText] = useState<string>("");
  const [isMicActive, setIsMicActive] = useState<boolean>(false);

  const inputRef = useRef<HTMLInputElement>(null);

  // Audio animation loop for simulating microphone or speech audio pulses
  useEffect(() => {
    let animId: number;
    let t = 0;

    const updateAudio = () => {
      if (state === "LISTENING" || isMicActive) {
        // Dynamic oscillating voice volume
        const level = 0.35 + Math.sin(t * 0.1) * 0.25 + Math.cos(t * 0.23) * 0.15;
        setAudioLevel(Math.max(0.1, Math.min(1.0, level)));
      } else if (state === "SPEAKING") {
        // Rhythmic speech pattern
        const level = 0.45 + Math.sin(t * 0.18) * 0.35;
        setAudioLevel(Math.max(0.15, Math.min(1.0, level)));
      } else if (state === "EXECUTING" || state === "PROCESSING") {
        // High-energy steady pulse
        setAudioLevel(0.4);
      } else {
        // Idle gentle breathing
        const level = 0.05 + Math.sin(t * 0.03) * 0.04;
        setAudioLevel(Math.max(0, level));
      }
      t++;
      animId = requestAnimationFrame(updateAudio);
    };

    animId = requestAnimationFrame(updateAudio);
    return () => cancelAnimationFrame(animId);
  }, [state, isMicActive]);

  // Listen for real-time Voice Events from Tauri Backend EventBus
  useEffect(() => {
    const unlistens: (() => void)[] = [];
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen<string>("jarvis-voice-state", (event) => {
          const st = event.payload as any;
          if (st === "WAKE_DETECTED") {
            setState("WAKE_DETECTED");
          } else if (st === "LISTENING") {
            setState("LISTENING");
          } else if (st === "SUCCESS") {
            setState("SUCCESS");
            setTimeout(() => setState("IDLE"), 1400);
          }
        }).then((u) => unlistens.push(u));

        listen<string>("jarvis-transcribed", (event) => {
          setTranscript(event.payload || "");
          setState("PROCESSING");
        }).then((u) => unlistens.push(u));

        listen<string>("jarvis-speaking", (event) => {
          setResponse(event.payload || "");
          setState("SPEAKING");
        }).then((u) => unlistens.push(u));
      });
    }
    return () => {
      unlistens.forEach((u) => u());
    };
  }, []);



  // Execute command through Tauri IPC / Real Architecture
  const executeCommand = useCallback(async (commandText: string) => {
    const text = commandText.trim();
    if (!text) return;

    setTranscript(text);
    setResponse("");
    setTelemetry(undefined);
    setState("WAKE_DETECTED");

    // Step 1: Wake detection transition
    await new Promise((r) => setTimeout(r, 450));
    setState("PROCESSING");

    await new Promise((r) => setTimeout(r, 550));
    setState("PLANNING");

    await new Promise((r) => setTimeout(r, 600));
    setState("EXECUTING");

    // Execute via Tauri or Real Backend
    const tauriResult: any = await invokeTauri("execute_command", { command: text });

    if (tauriResult) {
      if (tauriResult.Success) {
        const data = tauriResult.Success;
        setTelemetry({
          taskId: data.task_id,
          toolName: data.tool_name,
          applicationName: data.tool_data?.application,
          processId: data.tool_data?.pid,
          durationMs: data.duration_ms,
        });
        setState("SPEAKING");
        setResponse(data.spoken_response);
        await new Promise((r) => setTimeout(r, 3200));
        setState("SUCCESS");
        await new Promise((r) => setTimeout(r, 1400));
        setState("IDLE");
      } else if (tauriResult.ApprovalRequired) {
        setState("AWAITING_USER");
        setResponse(tauriResult.ApprovalRequired.reason);
      } else if (tauriResult.Denied) {
        setState("ERROR");
        setResponse(`Policy Denied: ${tauriResult.Denied.reason}`);
        await new Promise((r) => setTimeout(r, 3000));
        setState("IDLE");
      } else if (tauriResult.Failed) {
        setState("ERROR");
        setResponse(`Execution Error: ${tauriResult.Failed.error}`);
        await new Promise((r) => setTimeout(r, 3000));
        setState("IDLE");
      }
      return;
    }

    // Direct Browser / Dev Simulation following exact same contract
    if (text.toLowerCase().includes("chrome")) {
      setTelemetry({
        toolName: "open_application",
        applicationName: "chrome",
        processId: 14820,
        durationMs: 48,
      });
      await new Promise((r) => setTimeout(r, 800));
      setState("SPEAKING");
      setResponse("Chrome is open, sir.");
      await new Promise((r) => setTimeout(r, 3200));
      setState("SUCCESS");
      await new Promise((r) => setTimeout(r, 1400));
      setState("IDLE");
    } else if (text.toLowerCase().includes("time")) {
      setTelemetry({
        toolName: "get_time",
        durationMs: 2,
      });
      const now = new Date();
      const timeStr = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      const dateStr = now.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });
      setState("SPEAKING");
      setResponse(`It is currently ${timeStr}, ${dateStr}.`);
      await new Promise((r) => setTimeout(r, 3200));
      setState("IDLE");
    } else {
      setTelemetry({
        toolName: "open_application",
        applicationName: text,
        processId: 18204,
        durationMs: 65,
      });
      setState("SPEAKING");
      setResponse(`I have executed your request for ${text}, sir.`);
      await new Promise((r) => setTimeout(r, 3000));
      setState("SUCCESS");
      await new Promise((r) => setTimeout(r, 1200));
      setState("IDLE");
    }
  }, []);

  // Real-time Speech-to-Text Recognition Hook (Webview2 / Browser Native STT)
  useEffect(() => {
    if (state !== "LISTENING") return;

    const SpeechRecognition = (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition;
    if (!SpeechRecognition) {
      console.warn("SpeechRecognition API not available in current window context");
      return;
    }

    try {
      const recognition = new SpeechRecognition();
      recognition.continuous = false;
      recognition.interimResults = true;
      recognition.lang = "en-US";

      recognition.onresult = (event: any) => {
        let text = "";
        for (let i = 0; i < event.results.length; i++) {
          text += event.results[i][0].transcript;
        }
        if (text) {
          setTranscript(text);
          if (event.results[0].isFinal) {
            recognition.stop();
            executeCommand(text);
          }
        }
      };

      recognition.onerror = (err: any) => {
        console.warn("Speech recognition error:", err);
      };

      recognition.start();

      return () => {
        try {
          recognition.stop();
        } catch (_) {}
      };
    } catch (e) {
      console.warn("SpeechRecognition start error:", e);
    }
  }, [state, executeCommand]);

  const handleVoiceTrigger = async () => {
    if (state === "LISTENING" || state === "WAKE_DETECTED") {
      setState("IDLE");
      setIsMicActive(false);
      return;
    }

    setState("WAKE_DETECTED");
    setIsMicActive(true);
    setTranscript("");
    setResponse("");

    // Invoke Tauri backend wake word trigger if running inside Tauri desktop app
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("trigger_wake_word");
      } catch (err) {
        console.warn("Tauri wake word trigger warning:", err);
      }
    }

    setTimeout(() => {
      setState("LISTENING");
    }, 400);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && inputText.trim()) {
      e.preventDefault();
      const cmd = inputText.trim();
      setInputText("");
      executeCommand(cmd);
    }
  };

  return (
    <div className="jarvis-hud-viewport">
      {/* ── Background Blueprint & High-Tech Grid ──────── */}
      <div className="hud-background-grid" />
      <div className="hud-ambient-glow" />

      {/* ── Top Futuristic System Telemetry ───────────── */}
      <header className="hud-top-bar">
        <div className="hud-top-left">
          <span className="hud-tag">JARVIS // LOCAL CORE</span>
          <span className="hud-sys-status">SYS_STATUS: OPTIMAL</span>
        </div>
        <div className="hud-top-right">
          <span className="hud-tag">AUTONOMY: LVL 3 (CONSERVATIVE)</span>
          <span className="hud-tag">PLATFORM: WINDOWS 11</span>
        </div>
      </header>

      {/* ── Center Concentric Holographic HUD ─────────── */}
      <main className="hud-center-stage">
        <div className="hud-stage-inner">
          <HudSvg state={state} audioLevel={audioLevel} />
          <HudVisualizer state={state} audioLevel={audioLevel} />
          <HudCore state={state} audioLevel={audioLevel} />
        </div>
      </main>

      {/* ── Bottom Transcript, Action Readout & Controls ─ */}
      <footer className="hud-bottom-stage">
        <HudTranscript
          state={state}
          transcript={transcript}
          response={response}
          telemetry={telemetry}
          onApprove={() => executeCommand(transcript)}
          onDeny={() => {
            setState("IDLE");
            setResponse("Action aborted by user.");
          }}
        />

        {/* ── Minimal Voice & Command Input (Development Trigger) ── */}
        <div className="hud-controls-dock">
          {/* Voice Activation Orb */}
          <button
            id="hud-voice-trigger"
            className={`hud-voice-orb ${state === "LISTENING" ? "active-listening" : ""}`}
            onClick={handleVoiceTrigger}
            title="Voice Activation ('JARVIS')"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75">
              <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
              <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
              <line x1="12" y1="19" x2="12" y2="23" />
              <line x1="8" y1="23" x2="16" y2="23" />
            </svg>
          </button>

          {/* Quick Directive Chips */}
          <div className="hud-quick-directives">
            <button
              className="hud-directive-chip"
              onClick={() => executeCommand("open chrome")}
              disabled={state !== "IDLE"}
            >
              ▶ "open chrome"
            </button>
            <button
              className="hud-directive-chip"
              onClick={() => executeCommand("what time is it")}
              disabled={state !== "IDLE"}
            >
              ▶ "what time is it"
            </button>
          </div>

          {/* Hidden/Minimal Command Line Input */}
          <div className="hud-input-capsule">
            <input
              ref={inputRef}
              id="hud-command-input"
              type="text"
              className="hud-input-element"
              placeholder={state === "IDLE" ? "Type or speak command..." : state}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={state !== "IDLE"}
            />
            {inputText.trim() && (
              <button
                className="hud-input-send"
                onClick={() => {
                  const cmd = inputText.trim();
                  setInputText("");
                  executeCommand(cmd);
                }}
              >
                EXECUTE
              </button>
            )}
          </div>
        </div>
      </footer>
    </div>
  );
};
