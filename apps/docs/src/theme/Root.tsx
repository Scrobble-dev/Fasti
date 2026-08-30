import { useEffect, type ReactNode } from "react";

export default function Root({ children }: { children: ReactNode }): ReactNode {
  useEffect(() => {
    const root = document.documentElement;
    const sync = () => {
      root.dataset.bsTheme = root.dataset.theme === "dark" ? "dark" : "light";
    };
    sync();
    const observer = new MutationObserver(sync);
    observer.observe(root, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    const closeNavigation = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const close = document.querySelector<HTMLButtonElement>(
        ".navbar-sidebar__close",
      );
      if (!close || !document.querySelector(".navbar-sidebar--show")) return;
      close.click();
      document.querySelector<HTMLButtonElement>(".navbar__toggle")?.focus();
    };
    document.addEventListener("keydown", closeNavigation);
    return () => {
      observer.disconnect();
      document.removeEventListener("keydown", closeNavigation);
    };
  }, []);
  return children;
}
