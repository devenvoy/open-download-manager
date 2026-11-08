/* Shared capture rule evaluator for browser extension and native messaging */
(function(root) {
  function matchesHost(host, pattern) {
    if (!host || !pattern) return false;
    const h = host.toLowerCase();
    const pat = pattern.trim().toLowerCase().replace(/^\*\./, '');
    if (pattern === '*' || h === pat) return true;
    return h.endsWith('.' + pat);
  }

  function extractExtension(filename, pathname) {
    if (filename && typeof filename === 'string') {
      const idx = filename.lastIndexOf('.');
      if (idx > 0 && idx < filename.length - 1) {
        const ext = filename.slice(idx + 1).toLowerCase();
        if (/^[a-z0-9]+$/i.test(ext)) return ext;
      }
    }
    if (pathname && typeof pathname === 'string') {
      const segment = pathname.split('/').filter(Boolean).pop() || '';
      const idx = segment.lastIndexOf('.');
      if (idx > 0 && idx < segment.length - 1) {
        const ext = segment.slice(idx + 1).toLowerCase();
        if (/^[a-z0-9]+$/i.test(ext)) return ext;
      }
    }
    return null;
  }

  function evaluateCapture(policy, url, filename) {
    if (!policy || policy.enabled === false) {
      return { decision: 'type-not-enabled', reason: 'Capture rules disabled' };
    }

    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      return { decision: 'type-not-enabled', reason: 'Invalid URL format' };
    }

    const host = parsed.hostname.toLowerCase();
    const excludedHosts = Array.isArray(policy.excludedHosts) ? policy.excludedHosts : [];
    for (const pattern of excludedHosts) {
      if (matchesHost(host, pattern)) {
        return {
          decision: 'excluded-host',
          reason: `Host '${host}' matches exclusion '${pattern}'`
        };
      }
    }

    const excludedAddresses = Array.isArray(policy.excludedAddresses) ? policy.excludedAddresses : [];
    const fullUrlLower = url.toLowerCase();
    const pathLower = parsed.pathname.toLowerCase();
    const noScheme = fullUrlLower.replace(/^https?:\/\//, '');
    for (const pattern of excludedAddresses) {
      const pat = pattern.trim().toLowerCase();
      if (!pat) continue;
      if (
        fullUrlLower.startsWith(pat) ||
        pathLower.startsWith(pat) ||
        host === pat ||
        noScheme.startsWith(pat)
      ) {
        return {
          decision: 'excluded-address',
          reason: `Address matches exclusion '${pattern}'`
        };
      }
    }

    const mode = policy.confirmationMode || 'askForUnknown';
    if (mode === 'alwaysAsk') {
      return {
        decision: 'confirmation-required',
        reason: 'Confirmation mode is set to AlwaysAsk'
      };
    }

    const fileTypes = Array.isArray(policy.fileTypes) ? policy.fileTypes.map(t => t.toLowerCase().replace(/^\./, '')) : [];
    const ext = extractExtension(filename, parsed.pathname);

    if (ext && fileTypes.includes(ext)) {
      return { decision: 'allowed', reason: `Extension '.${ext}' is allowed` };
    }

    if (mode === 'askForUnknown') {
      return {
        decision: 'confirmation-required',
        reason: ext ? `Extension '.${ext}' requires confirmation` : 'Unknown file type requires confirmation'
      };
    }

    return {
      decision: 'type-not-enabled',
      reason: ext ? `Extension '.${ext}' not in inclusion list` : 'No recognized file extension'
    };
  }

  const api = {
    matchesHost,
    extractExtension,
    evaluateCapture
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.captureEvaluator = api;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
