import React, { useEffect, useRef } from "react";
import { HudState } from "../../types/hud";

interface HudVisualizerProps {
  state: HudState;
  audioLevel: number;
}

/**
 * Canvas-based Real-Time Audio Frequency Wave & Orbital Ring Visualizer
 * Reacts to microphone or synthesized speech audio levels.
 */
export const HudVisualizer: React.FC<HudVisualizerProps> = ({ state, audioLevel }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let animationFrameId: number;
    let phase = 0;

    const render = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      const centerX = canvas.width / 2;
      const centerY = canvas.height / 2;
      const baseRadius = 145;

      const isVoiceActive = state === "LISTENING" || state === "SPEAKING";
      const waveCount = isVoiceActive ? 3 : 1;
      const amplitude = isVoiceActive ? Math.max(audioLevel * 25, 4) : 2;

      ctx.save();

      for (let w = 0; w < waveCount; w++) {
        ctx.beginPath();
        const segments = 120;
        const currentRadius = baseRadius + w * 12;

        for (let i = 0; i <= segments; i++) {
          const angle = (i / segments) * Math.PI * 2;
          // Harmonic wave distortion
          const frequency = 6 + w * 2;
          const offset = Math.sin(angle * frequency + phase + w) * amplitude;
          const r = currentRadius + offset;

          const x = centerX + Math.cos(angle) * r;
          const y = centerY + Math.sin(angle) * r;

          if (i === 0) {
            ctx.moveTo(x, y);
          } else {
            ctx.lineTo(x, y);
          }
        }

        ctx.closePath();

        if (state === "ERROR") {
          ctx.strokeStyle = `rgba(255, 60, 90, ${0.7 - w * 0.2})`;
        } else if (state === "SUCCESS") {
          ctx.strokeStyle = `rgba(0, 255, 180, ${0.8 - w * 0.2})`;
        } else if (state === "AWAITING_USER") {
          ctx.strokeStyle = `rgba(255, 170, 40, ${0.8 - w * 0.2})`;
        } else {
          ctx.strokeStyle = `rgba(0, 212, 255, ${0.75 - w * 0.25})`;
        }

        ctx.lineWidth = 1.5;
        ctx.shadowColor = ctx.strokeStyle;
        ctx.shadowBlur = 8;
        ctx.stroke();
      }

      ctx.restore();

      phase += isVoiceActive ? 0.08 : 0.02;
      animationFrameId = requestAnimationFrame(render);
    };

    render();

    return () => {
      cancelAnimationFrame(animationFrameId);
    };
  }, [state, audioLevel]);

  return (
    <canvas
      ref={canvasRef}
      width={600}
      height={600}
      className="hud-visualizer-canvas"
    />
  );
};
