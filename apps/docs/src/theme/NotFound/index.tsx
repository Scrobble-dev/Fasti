import Link from "@docusaurus/Link";
import Layout from "@theme/Layout";

export default function NotFound(): React.JSX.Element {
  return (
    <Layout
      title="Page not found"
      description="Recovery options for a missing Fasti documentation page."
    >
      <main className="fasti-main">
        <h1>Page not found</h1>
        <p>The address does not match a Fasti documentation page.</p>
        <div className="fasti-actions">
          <Link className="btn btn-primary" to="/start/choose-a-path/">
            Choose a task path
          </Link>
          <Link className="btn btn-outline-primary" to="/search/">
            Search documentation
          </Link>
        </div>
        <p>
          If a Fasti link brought you here, report the broken link in the{" "}
          <a href="https://github.com/Scrobble-dev/Fasti/issues">
            Fasti issue tracker
          </a>
          .
        </p>
      </main>
    </Layout>
  );
}
