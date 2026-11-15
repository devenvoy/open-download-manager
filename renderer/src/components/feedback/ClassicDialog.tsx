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
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 backdrop-blur-[0.5px] p-4 overflow-y-auto animate-in fade-in duration-100"
      onClick={(e) => {
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
      role="presentation"
    >
      <dialog
        id={id}
        className={cn(
          "dialog relative m-0 max-h-[90vh] overflow-auto shadow-2xl rounded-sm border border-[#8b8b8b] dark:border-[#383838]",
          className,
        )}
        open
        aria-labelledby={titleId}
        aria-label={ariaLabel}
        onClick={(e) => e.stopPropagation()}
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
    </div>
  );
}
