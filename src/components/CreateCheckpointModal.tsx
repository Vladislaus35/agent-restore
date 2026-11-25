import { useState } from "react";
import { PlusCircle, X } from "lucide-react";
import type { TranslationDict } from "../i18n/translations";

interface CreateCheckpointModalProps {
  t: TranslationDict;
  onClose: () => void;
  onCreate: (label: string, note: string) => Promise<void>;
}

export function CreateCheckpointModal({ t, onClose, onCreate }: CreateCheckpointModalProps) {
  const [label, setLabel] = useState("");
  const [note, setNote] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      await onCreate(label.trim(), note.trim());
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" style={{ maxWidth: 480 }} onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>{t.createModal.title}</h2>
          <button className="btn btn-xs btn-ghost" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="modal-body">
            <div className="field">
              <label>{t.createModal.labelField}</label>
              <input
                className="input"
                autoFocus
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder={t.createModal.labelPlaceholder}
              />
            </div>

            <div className="field">
              <label>{t.createModal.noteField}</label>
              <textarea
                className="textarea"
                rows={3}
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t.createModal.notePlaceholder}
              />
            </div>
          </div>

          <div className="modal-footer">
            <span />
            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" className="btn btn-sm" onClick={onClose} disabled={submitting}>
                {t.createModal.cancel}
              </button>
              <button type="submit" className="btn btn-sm btn-primary" disabled={submitting}>
                <PlusCircle size={13} />
                {submitting ? t.createModal.creating : t.createModal.create}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
