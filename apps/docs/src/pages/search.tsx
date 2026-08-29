import Layout from "@theme/Layout";
import { useEffect } from "react";

declare global {
  interface Window {
    PagefindUI?: new (options: {
      element: string;
      showSubResults: boolean;
    }) => unknown;
  }
}

export default function Search(): React.JSX.Element {
  useEffect(() => {
    const initialise = () => {
      if (window.PagefindUI)
        new window.PagefindUI({
          element: "#pagefind-search",
          showSubResults: true,
        });
    };
    const script = document.createElement("script");
    script.src = "/pagefind/pagefind-ui.js";
    script.onload = initialise;
    document.head.append(script);
    const stylesheet = document.createElement("link");
    stylesheet.rel = "stylesheet";
    stylesheet.href = "/pagefind/pagefind-ui.css";
    document.head.append(stylesheet);
    return () => {
      script.remove();
      stylesheet.remove();
    };
  }, []);
  return (
    <Layout
      title="Search"
      description="Search the local static Fasti documentation index."
    >
      <main className="fasti-main">
        <h1>Search</h1>
        <p>
          The query stays in this browser. The site sends no search or telemetry
          request.
        </p>
        <div id="pagefind-search" role="search" />
      </main>
    </Layout>
  );
}
