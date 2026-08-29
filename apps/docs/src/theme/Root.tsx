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
    return () => observer.disconnect();
  }, []);
  return children;
}
