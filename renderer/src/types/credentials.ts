/** Credential rule DTO — mirrors src-tauri/src/domain/credentials/rule.rs. */

/** `CredentialRule` — a stored credential matched by scheme, host and path. */
export interface CredentialRule {
  id: string;
  scheme: string;
  host: string;
  port: number | null;
  pathPrefix: string;
  username: string;
  secret: string;
}
