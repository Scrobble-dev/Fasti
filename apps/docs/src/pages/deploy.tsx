import {
  createDeploymentPlan,
  deploymentModes,
  renderPosixCommand,
  type DeploymentMode,
} from "@fasti/deploy-plan";
import Layout from "@theme/Layout";
import { useMemo, useState } from "react";

const initial = {
  mode: "native" as DeploymentMode,
  port: 8420,
  dataRoot: "/path/to/private/fasti-data",
  publicUrl: "https://fasti.internal",
};

export default function Deploy(): React.JSX.Element {
  const [input, setInput] = useState(initial);
  const [copyStatus, setCopyStatus] = useState("");
  const plan = useMemo(() => createDeploymentPlan(input), [input]);
  const command = renderPosixCommand(plan);
  async function copy(): Promise<void> {
    await navigator.clipboard.writeText(command);
    setCopyStatus("Command copied.");
  }
  return (
    <Layout
      title="Experimental deployment planner"
      description="Generate a bounded Fasti local review plan without collecting credentials."
    >
      <main className="fasti-main fasti-planner">
        <h1>Experimental deployment planner</h1>
        <p>
          This planner creates review commands in the browser. It sends no data
          and collects no credentials. Fasti has no supported production
          profile.
        </p>
        <form onSubmit={(event) => event.preventDefault()}>
          <fieldset>
            <legend>Review mode</legend>
            <div className="fasti-mode-list">
              {deploymentModes.map((mode) => (
                <label key={mode}>
                  <input
                    type="radio"
                    name="mode"
                    value={mode}
                    checked={input.mode === mode}
                    onChange={() => setInput({ ...input, mode })}
                  />{" "}
                  {mode === "trusted-proxy"
                    ? "Trusted HTTPS proxy"
                    : mode[0].toUpperCase() + mode.slice(1)}
                </label>
              ))}
            </div>
          </fieldset>
          <div className="mb-3">
            <label className="form-label" htmlFor="port">
              Port
            </label>
            <input
              className="form-control"
              id="port"
              type="number"
              min="1024"
              max="65535"
              value={input.port}
              onChange={(event) =>
                setInput({ ...input, port: Number(event.target.value) })
              }
            />
          </div>
          <div className="mb-3">
            <label className="form-label" htmlFor="data-root">
              Private data root
            </label>
            <input
              className="form-control"
              id="data-root"
              value={input.dataRoot}
              onChange={(event) =>
                setInput({ ...input, dataRoot: event.target.value })
              }
              autoComplete="off"
            />
          </div>
          {input.mode === "trusted-proxy" && (
            <div className="mb-3">
              <label className="form-label" htmlFor="public-url">
                Public HTTPS URL
              </label>
              <input
                className="form-control"
                id="public-url"
                type="url"
                value={input.publicUrl}
                onChange={(event) =>
                  setInput({ ...input, publicUrl: event.target.value })
                }
                autoComplete="off"
              />
            </div>
          )}
          <button
            className="btn btn-secondary"
            type="button"
            onClick={() => {
              setInput(initial);
              setCopyStatus("");
            }}
          >
            Reset plan
          </button>
        </form>
        <section
          aria-live="polite"
          aria-labelledby="plan-heading"
          className="fasti-plan-output"
        >
          <h2 id="plan-heading">{plan.label}</h2>
          <p>
            <strong>
              {plan.available ? "Available for bounded review" : "Unavailable"}
            </strong>
          </p>
          {plan.blockers.length > 0 && (
            <div className="alert alert-danger" role="alert">
              <h3>Blockers</h3>
              <ul>
                {plan.blockers.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </div>
          )}
          {plan.warnings.map((warning) => (
            <p className="alert alert-warning" key={warning}>
              {warning}
            </p>
          ))}
          {command && (
            <>
              <h3>Command</h3>
              <pre tabIndex={0}>
                <code>{command}</code>
              </pre>
              <button className="btn btn-primary" type="button" onClick={copy}>
                Copy command
              </button>
              <span className="fasti-copy-status" role="status">
                {copyStatus}
              </span>
            </>
          )}
          {plan.verification.length > 0 && (
            <>
              <h3>Verify</h3>
              <ol>
                {plan.verification.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ol>
              <h3>Stop and recover</h3>
              <ol>
                {plan.rollback.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ol>
            </>
          )}
        </section>
      </main>
    </Layout>
  );
}
