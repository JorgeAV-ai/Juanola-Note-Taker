import { Component, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { t } from "@/lib/i18n";

// Si algo falla al pintar, React desmonta toda la app y la ventana queda en
// blanco. Esto muestra el error con un botón para recargar y lo deja en el log.
export class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack?: string | null }) {
    const line = `${error.message}\n${info.componentStack ?? ""}`;
    console.error(line);
    invoke("frontend_error", { message: line }).catch(() => {});
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 bg-canvas p-10 text-center">
        <h1 className="font-serif text-[30px] font-normal">{t("error.title")}</h1>
        <p className="max-w-[520px] font-mono text-[12.5px] text-muted">{this.state.error.message}</p>
        <button className="h-9 rounded-full border border-line bg-surface px-4 text-[14px] hover:bg-hover" onClick={() => location.reload()}>
          {t("error.reload")}
        </button>
      </div>
    );
  }
}
