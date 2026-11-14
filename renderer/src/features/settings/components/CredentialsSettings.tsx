import { useState } from "react";
import {
  useCredentialRules,
  useDeleteCredentialRule,
  useSaveCredentialRule,
} from "../hooks/use-settings";

export function CredentialsSettings() {
  const { data: rules } = useCredentialRules();
  const saveMutation = useSaveCredentialRule();
  const deleteMutation = useDeleteCredentialRule();

  const [scheme, setScheme] = useState("https");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [path, setPath] = useState("/");
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [status, setStatus] = useState<{
    type: "idle" | "success" | "error";
    message: string;
  }>({ type: "idle", message: "" });

  async function handleAddRule() {
    const trimmedHost = host.trim();
    const trimmedUser = user.trim();

    if (!trimmedHost) {
      setStatus({ type: "error", message: "Please enter a valid host." });
      return;
    }
    if (!trimmedUser) {
      setStatus({ type: "error", message: "Please enter a username." });
      return;
    }

    const portNum = port.trim() ? parseInt(port.trim(), 10) : null;
    const ruleId = crypto.randomUUID();

    try {
      await saveMutation.mutateAsync({
        id: ruleId,
        scheme,
        host: trimmedHost,
        port: portNum,
        pathPrefix: path.trim() || "/",
        username: trimmedUser,
        secret: pass,
      });

      setHost("");
      setPort("");
      setPath("/");
      setUser("");
      setPass("");
      setStatus({ type: "success", message: "Login rule saved." });
      setTimeout(() => setStatus({ type: "idle", message: "" }), 3000);
    } catch (err) {
      setStatus({
        type: "error",
        message: err instanceof Error ? err.message : "Failed to save rule.",
      });
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteMutation.mutateAsync(id);
    } catch (err) {
      setStatus({
        type: "error",
        message: `Failed to delete rule: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }

  return (
    <fieldset id="credentials-fieldset">
      <legend>Site Logins &amp; Passwords</legend>
      <p className="field-help">
        Manage saved credentials for protected HTTP and FTP downloads.
        Longest-prefix matching will automatically select the right credentials
        without leaking across hosts or out-of-scope paths.
      </p>

      <div className="credentials-manager">
        <div
          className="credentials-table-wrapper"
          style={{
            maxHeight: 140,
            overflowY: "auto",
            border: "1px solid var(--border-color, #ccc)",
            marginBottom: 8,
          }}
        >
          <table
            className="credentials-table"
            id="credentials-table"
            style={{ width: "100%", borderCollapse: "collapse", fontSize: 11 }}
          >
            <thead>
              <tr
                style={{
                  background: "var(--bg-secondary, #f3f4f6)",
                  textAlign: "left",
                }}
              >
                <th style={{ padding: 4 }}>Scheme</th>
                <th style={{ padding: 4 }}>Host</th>
                <th style={{ padding: 4 }}>Port</th>
                <th style={{ padding: 4 }}>Path Prefix</th>
                <th style={{ padding: 4 }}>User</th>
                <th style={{ padding: 4 }}>Password</th>
                <th style={{ padding: 4 }}>Action</th>
              </tr>
            </thead>
            <tbody id="credentials-list">
              {(rules ?? []).length === 0 ? (
                <tr>
                  <td
                    colSpan={7}
                    style={{
                      padding: 8,
                      textAlign: "center",
                      color: "var(--muted-text, #666)",
                    }}
                  >
                    No login rules saved.
                  </td>
                </tr>
              ) : (
                (rules ?? []).map((r) => (
                  <tr
                    key={r.id}
                    style={{
                      borderBottom: "1px solid var(--border-color, #eee)",
                    }}
                  >
                    <td style={{ padding: 4 }}>{r.scheme}</td>
                    <td style={{ padding: 4 }}>{r.host}</td>
                    <td style={{ padding: 4 }}>{r.port ?? "any"}</td>
                    <td style={{ padding: 4 }}>{r.pathPrefix}</td>
                    <td style={{ padding: 4 }}>{r.username}</td>
                    <td style={{ padding: 4 }}>••••••••</td>
                    <td style={{ padding: 4 }}>
                      <button
                        type="button"
                        className="small-button"
                        onClick={() => void handleDelete(r.id)}
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        <div
          className="credentials-add-form"
          id="credentials-add-form"
          style={{ display: "flex", gap: 4, alignItems: "center", flexWrap: "wrap" }}
        >
          <select
            id="cred-scheme"
            value={scheme}
            onChange={(e) => setScheme(e.target.value)}
            style={{ fontSize: 11, padding: 2 }}
          >
            <option value="https">https</option>
            <option value="http">http</option>
            <option value="ftp">ftp</option>
          </select>
          <input
            type="text"
            id="cred-host"
            placeholder="Host (e.g. site.com)"
            value={host}
            onChange={(e) => setHost(e.target.value)}
            style={{ fontSize: 11, padding: 2, width: 110 }}
          />
          <input
            type="number"
            id="cred-port"
            placeholder="Port"
            min={1}
            max={65535}
            value={port}
            onChange={(e) => setPort(e.target.value)}
            style={{ fontSize: 11, padding: 2, width: 50 }}
          />
          <input
            type="text"
            id="cred-path"
            placeholder="Path (e.g. /team/)"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            style={{ fontSize: 11, padding: 2, width: 80 }}
          />
          <input
            type="text"
            id="cred-user"
            placeholder="User"
            autoComplete="off"
            value={user}
            onChange={(e) => setUser(e.target.value)}
            style={{ fontSize: 11, padding: 2, width: 80 }}
          />
          <input
            type="password"
            id="cred-pass"
            placeholder="Password"
            autoComplete="off"
            value={pass}
            onChange={(e) => setPass(e.target.value)}
            style={{ fontSize: 11, padding: 2, width: 80 }}
          />
          <button
            type="button"
            className="small-button"
            id="cred-save-btn"
            disabled={saveMutation.isPending}
            onClick={() => void handleAddRule()}
          >
            Add Rule
          </button>
        </div>

        {status.message && (
          <span
            id="cred-status"
            className="field-help"
            style={{
              display: "block",
              marginTop: 4,
              color: status.type === "error" ? "red" : "green",
            }}
          >
            {status.message}
          </span>
        )}
      </div>
    </fieldset>
  );
}
