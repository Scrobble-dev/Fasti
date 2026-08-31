import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";
import { useEffect, useState } from "react";

declare global {
  interface Window {
    PagefindUI?: new (options: {
      element: string;
      showSubResults: boolean;
    }) => unknown;
  }
}

export default function Search(): React.JSX.Element {
  const [ready, setReady] = useState(false);
  const [loadError, setLoadError] = useState(false);
  useEffect(() => {
    let mounted = true;
    let scriptReady = false;
    let stylesheetReady = false;
    const fail = () => {
      if (mounted) setLoadError(true);
    };
    const initialise = () => {
      if (!mounted || !scriptReady || !stylesheetReady) return;
      const target = document.querySelector("#pagefind-search");
      if (!window.PagefindUI || !target) {
        fail();
        return;
      }
      new window.PagefindUI({
        element: "#pagefind-search",
        showSubResults: true,
      });
      const input = document.querySelector<HTMLInputElement>(
        "#pagefind-search input",
      );
      input?.setAttribute("aria-label", "Search documentation");
      input?.setAttribute("role", "searchbox");
      setReady(true);
    };
    const script = document.createElement("script");
    script.src = "/pagefind/pagefind-ui.js";
    script.onload = () => {
      scriptReady = true;
      initialise();
    };
    script.onerror = fail;
    document.head.append(script);
    const stylesheet = document.createElement("link");
    stylesheet.rel = "stylesheet";
    stylesheet.href = "/pagefind/pagefind-ui.css";
    stylesheet.onload = () => {
      stylesheetReady = true;
      initialise();
    };
    stylesheet.onerror = fail;
    document.head.append(stylesheet);
    return () => {
      mounted = false;
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
        {!ready && (
          <div
            aria-busy={!loadError}
            className="fasti-search-fallback"
            role="search"
          >
            <label className="visually-hidden" htmlFor="fasti-search-fallback">
              Search documentation
            </label>
            <input
              aria-describedby={loadError ? undefined : "fasti-search-loading"}
              className="form-control"
              disabled
              id="fasti-search-fallback"
              placeholder="Search"
              type="search"
            />
            {!loadError && (
              <span
                className="visually-hidden"
                id="fasti-search-loading"
                role="status"
              >
                Search is loading.
              </span>
            )}
          </div>
        )}
        <div
          className={ready ? undefined : "fasti-search-pending"}
          id="pagefind-search"
          role="search"
        />
        {loadError && (
          <div className="alert alert-danger" role="alert">
            Local search could not load.{" "}
            <Link to="/start/choose-a-path/">Choose a path.</Link>
          </div>
        )}
        <noscript>
          <div className="alert alert-danger">
            Search needs JavaScript.{" "}
            <a href="/start/choose-a-path/">Choose a path.</a>
          </div>
        </noscript>
      </main>
    </Layout>
  );
}
