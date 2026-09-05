import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import { useEffect, useState } from "react";

type Capability = {
  lifecycle: { contract_state: string; runtime_availability: string };
};

export default function Status(): React.JSX.Element {
  const [capabilities, setCapabilities] = useState<Capability[] | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    fetch("/capabilities.json")
      .then((response) => {
        if (!response.ok) throw new Error();
        return response.json();
      })
      .then((value) => setCapabilities(value.capabilities))
      .catch(() => setError(true));
  }, []);
  const count = (
    field: "contract_state" | "runtime_availability",
    value: string,
  ) =>
    capabilities?.filter((capability) => capability.lifecycle[field] === value)
      .length ?? 0;
  return (
    <Layout
      title="Capability status"
      description="Generated Fasti contract and runtime state summary."
    >
      <main className="fasti-main">
        <h1>Capability status</h1>
        <p>
          Support, runtime, and contract states are separate. Fasti has no
          supported public release.
        </p>
        {error && (
          <div className="alert alert-danger" role="alert">
            The generated capability data could not load.{" "}
            <Link to="/capabilities.json">Open the raw registry.</Link>
          </div>
        )}
        <table aria-busy={!error && capabilities === null} className="table">
          <caption>Current generated capability states</caption>
          <thead>
            <tr>
              <th scope="col">Dimension</th>
              <th scope="col">State</th>
              <th scope="col">Count</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th scope="row">Contract</th>
              <td>finalized</td>
              <td>
                {capabilities === null
                  ? "—"
                  : count("contract_state", "finalized")}
              </td>
            </tr>
            <tr>
              <th scope="row">Runtime</th>
              <td>implemented</td>
              <td>
                {capabilities === null
                  ? "—"
                  : count("runtime_availability", "implemented")}
              </td>
            </tr>
            <tr>
              <th scope="row">Runtime</th>
              <td>unavailable</td>
              <td>
                {capabilities === null
                  ? "—"
                  : count("runtime_availability", "unavailable")}
              </td>
            </tr>
          </tbody>
        </table>
        {!error && capabilities === null && (
          <span className="visually-hidden" role="status">
            Loading generated capability data.
          </span>
        )}
        <p>
          <Link to="/reference/capabilities/">
            Read every capability dimension.
          </Link>
        </p>
      </main>
    </Layout>
  );
}
