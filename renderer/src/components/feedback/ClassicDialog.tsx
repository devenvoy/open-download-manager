import { type ReactNode, useEffect } from "react";
import { cn } from "@/lib/utils";

export interface ClassicDialogProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  id?: string;
  className?: string;
  titleId?: string;
  ariaLabel?: string;
  children: ReactNode;
}

/**
 * Reusable modal dialog matching the authentic Windows IDM chrome.
 * Handles the title bar, close button, Escape key, and conditional rendering.
 */
export function ClassicDialog({
  open,
  onClose,
  title,
  id,
  className,
  titleId,
  ariaLabel,
  children,
}: ClassicDialogProps) {
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <dialog
      id={id}
      className={cn("dialog", className)}
      open
      aria-labelledby={titleId}
      aria-label={ariaLabel}
    >
      <div className="dialog-title">
        {typeof title === "string" ? (
          <strong id={titleId}>{title}</strong>
        ) : (
          title
        )}
        <button
          type="button"
          className="close-button"
          data-close
          aria-label="Close"
          onClick={onClose}
        >
          ×
        </button>
      </div>
      {children}
    </dialog>
  );
}
