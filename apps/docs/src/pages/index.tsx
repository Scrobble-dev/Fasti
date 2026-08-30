import Head from "@docusaurus/Head";
import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";

export default function Home(): React.JSX.Element {
  const structuredData = {
    "@context": "https://schema.org",
    "@type": "WebSite",
    name: "Fasti Documentation",
    url: "https://fasti.scrobble.dev/",
    description:
      "Implementation-aware documentation for Fasti, a local system of record for portable media activity.",
  };
  return (
    <Layout
      title="Documentation"
      description="Implementation-aware Fasti documentation for users, operators, integrators, extension authors, and contributors."
    >
      <Head>
        <script type="application/ld+json">
          {JSON.stringify(structuredData)}
        </script>
      </Head>
      <main className="fasti-main">
        <header className="fasti-hero">
          <h1>Keep media activity portable.</h1>
          <p className="fasti-lede">
            Fasti records what sources report. It keeps identity, progress,
            ratings, lists, collections, and synchronization operations
            distinct.
          </p>
          <div className="fasti-actions">
            <Link className="btn btn-primary" to="/start/choose-a-path/">
              Choose a task path
            </Link>
            <Link
              className="btn btn-outline-primary"
              to="/start/current-status/"
            >
              Read current status
            </Link>
          </div>
        </header>
        <section aria-labelledby="paths-heading" className="fasti-section">
          <h2 id="paths-heading">Start from the intended outcome</h2>
          <div className="fasti-path-list">
            <Path
              title="Use Fasti"
              href="/use/keep-a-local-record/"
              text="Understand stable local Records and provider evidence."
            />
            <Path
              title="Operate and recover"
              href="/operate/local-review/"
              text="Run a bounded local review and check durable-route state."
            />
            <Path
              title="Integrate and automate"
              href="/integrate/first-observation/"
              text="Trace a governed observation through the public contract."
            />
            <Path
              title="Map and extend"
              href="/extend/first-provider/"
              text="Review provider and namespace boundaries before an adapter change."
            />
            <Path
              title="Contribute and verify"
              href="/contribute/first-change/"
              text="Make one bounded change and produce exact-source evidence."
            />
          </div>
        </section>
      </main>
    </Layout>
  );
}

function Path({
  title,
  href,
  text,
}: {
  title: string;
  href: string;
  text: string;
}): React.JSX.Element {
  return (
    <article className="fasti-path">
      <h3>
        <Link to={href}>{title}</Link>
      </h3>
      <p>{text}</p>
    </article>
  );
}
