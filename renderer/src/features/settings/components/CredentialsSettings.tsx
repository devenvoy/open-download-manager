import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  useCredentialRules,
  useDeleteCredentialRule,
  useSaveCredentialRule,
} from "../hooks/use-settings";

export function CredentialsSettings() {
  const { data: rules, isPending } = useCredentialRules();
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
        message: `Failed to delete credential rule: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }

  return (
    <fieldset id="credentials-fieldset" className="rounded-md border p-4 space-y-4">
      <legend className="px-2 text-sm font-semibold">
        Site Logins &amp; Passwords
      </legend>
      <p className="text-xs text-muted-foreground">
        Manage saved credentials for protected HTTP and FTP downloads.
        Longest-prefix matching will automatically select the right
        credentials without leaking across hosts or out-of-scope paths.
      </p>

      <div className="credentials-manager space-y-3">
        <div className="rounded border overflow-x-auto max-h-40">
          <table
            className="w-full text-xs text-left border-collapse"
            id="credentials-table"
          >
            <thead>
              <tr className="bg-muted border-b">
                <th className="p-2 font-medium">Scheme</th>
                <th className="p-2 font-medium">Host</th>
                <th className="p-2 font-medium">Port</th>
                <th className="p-2 font-medium">Path Prefix</th>
                <th className="p-2 font-medium">User</th>
                <th className="p-2 font-medium">Password</th>
                <th className="p-2 font-medium text-right">Action</th>
              </tr>
            </thead>
            <tbody id="credentials-list">
              {isPending ? (
                <tr>
                  <td colSpan={7} className="p-4 text-center text-muted-foreground">
                    Loading credentials…
                  </td>
                </tr>
              ) : !rules || rules.length === 0 ? (
                <tr>
                  <td colSpan={7} className="p-4 text-center text-muted-foreground">
                    No stored site credentials.
                  </td>
                </tr>
              ) : (
                rules.map((rule) => (
                  <tr key={rule.id} className="border-b last:border-0 hover:bg-muted/30">
                    <td className="p-2">{rule.scheme}</td>
                    <td className="p-2 font-mono">{rule.host}</td>
                    <td className="p-2 font-mono">{rule.port ?? "—"}</td>
                    <td className="p-2 font-mono">{rule.pathPrefix}</td>
                    <td className="p-2">{rule.username}</td>
                    <td className="p-2 font-mono">******</td>
                    <td className="p-2 text-right">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="delete-cred-btn h-6 px-2 text-destructive"
                        data-id={rule.id}
                        disabled={deleteMutation.isPending}
                        onClick={() => void handleDelete(rule.id)}
                      >
                        Delete
                      </Button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        <div
          id="credentials-add-form"
          className="flex flex-wrap items-center gap-2 pt-1"
        >
          <Select value={scheme} onValueChange={setScheme}>
            <SelectTrigger id="cred-scheme" className="w-24 h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="https">https</SelectItem>
              <SelectItem value="http">http</SelectItem>
              <SelectItem value="ftp">ftp</SelectItem>
            </SelectContent>
          </Select>
          <Input
            id="cred-host"
            placeholder="Host (e.g. site.com)"
            value={host}
            className="w-36 h-8 text-xs"
            onChange={(e) => setHost(e.target.value)}
          />
          <Input
            id="cred-port"
            type="number"
            min={1}
            max={65535}
            placeholder="Port"
            value={port}
            className="w-20 h-8 text-xs"
            onChange={(e) => setPort(e.target.value)}
          />
          <Input
            id="cred-path"
            placeholder="Path (e.g. /team/)"
            value={path}
            className="w-28 h-8 text-xs"
            onChange={(e) => setPath(e.target.value)}
          />
          <Input
            id="cred-user"
            placeholder="User"
            value={user}
            autoComplete="off"
            className="w-28 h-8 text-xs"
            onChange={(e) => setUser(e.target.value)}
          />
          <Input
            id="cred-pass"
            type="password"
            placeholder="Password"
            value={pass}
            autoComplete="off"
            className="w-28 h-8 text-xs"
            onChange={(e) => setPass(e.target.value)}
          />
          <Button
            type="button"
            id="cred-save-btn"
            size="sm"
            className="h-8 text-xs"
            disabled={saveMutation.isPending}
            onClick={() => void handleAddRule()}
          >
            Add Rule
          </Button>
        </div>

        {status.message && (
          <span
            id="cred-status"
            className={`block text-xs ${
              status.type === "success"
                ? "text-green-600 dark:text-green-400"
                : "text-destructive"
            }`}
          >
            {status.message}
          </span>
        )}
      </div>
    </fieldset>
  );
}
