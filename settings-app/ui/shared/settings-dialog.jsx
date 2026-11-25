import React, { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

export function SettingsDialog({ id, title, open, onClose, children, className = "" }) {
  const backdrop = useRef(null), dialog = useRef(null), close = useRef(null), controller = useRef(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  useEffect(() => {
    controller.current = window.BlobUI.createModalController({
      backdrop: backdrop.current, dialog: dialog.current, initialFocus: () => close.current,
      inertTargets: [document.getElementById("settings-shell"), document.getElementById("title-drag-region")].filter(Boolean),
      onClose: () => onCloseRef.current(),
    });
    return () => { controller.current?.destroy(); controller.current = null; };
  }, []);
  useEffect(() => {
    if (!open || !dialog.current || !window.BlobUI?.Scrollbar) return undefined;
    const scrollbar = window.BlobUI.Scrollbar.attach(dialog.current);
    return () => scrollbar.destroy();
  }, [open]);
  useEffect(() => {
    if (open) controller.current?.open();
    else if (controller.current?.isOpen()) controller.current.close();
  }, [open]);
  return createPortal(<div ref={backdrop} className="confirm-backdrop" hidden>
    <section ref={dialog} id={id} className={`confirm-dialog ${className}`} role="dialog" aria-modal="true" aria-labelledby={`${id}-title`} tabIndex={-1}>
      <header className="calendar-modal-header">
        <h2 id={`${id}-title`}>{title}</h2>
        <button ref={close} type="button" className="neutral-icon-button calendar-modal-close" aria-label={`Close ${title}`} onClick={() => controller.current?.close()}>×</button>
      </header>
      {children}
    </section>
  </div>, document.body);
}
