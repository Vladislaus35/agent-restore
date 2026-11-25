import { X } from "lucide-react";
import type { ToastMessage } from "../lib/types";

interface ToastContainerProps {
  toasts: ToastMessage[];
  onDismiss: (id: string) => void;
}

export function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  if (toasts.length === 0) return null;
  return (
    <div className="toast-stack" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast ${toast.kind}`}>
          <div style={{ minWidth: 0 }}>
            <div style={{ fontWeight: 600, fontSize: 12.5 }}>{toast.title}</div>
            {toast.detail && (
              <div style={{ fontSize: 11.5, color: "var(--text-secondary)", marginTop: 2 }}>
                {toast.detail}
              </div>
            )}
          </div>
          <button
            className="btn btn-xs btn-ghost"
            onClick={() => onDismiss(toast.id)}
            aria-label="Close"
          >
            <X size={13} />
          </button>
        </div>
      ))}
    </div>
  );
}
