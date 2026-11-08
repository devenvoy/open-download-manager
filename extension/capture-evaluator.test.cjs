// Created: 2026-11-08
// Reason: feat(browser): add configurable context menu actions & file capture rules (#29)
// Last updated: 2026-11-08
// Reason: feat(browser): add configurable context menu actions & file capture rules (#29)
// Description: Unit tests verifying download capture evaluation across inclusion lists, exclusion lists, and confirmation modes.

const { describe, it, expect } = require('bun:test');
const { evaluateCapture, matchesHost, extractExtension } = require('./capture-evaluator.js');

describe('captureEvaluator', () => {
  const basePolicy = {
    enabled: true,
    fileTypes: ['zip', 'exe', 'pdf', 'mp4', 'tar', 'gz'],
    excludedHosts: ['bank.example.test', '*.internal.corp'],
    excludedAddresses: ['https://example.test/private/', '/secret/'],
    confirmationMode: 'askForUnknown'
  };

  it('matches host patterns accurately with subdomains', () => {
    expect(matchesHost('bank.example.test', 'bank.example.test')).toBe(true);
    expect(matchesHost('app.internal.corp', '*.internal.corp')).toBe(true);
    expect(matchesHost('sub.app.internal.corp', '*.internal.corp')).toBe(true);
    expect(matchesHost('other.corp', '*.internal.corp')).toBe(false);
  });

  it('extracts extension from filename or pathname', () => {
    expect(extractExtension('archive.zip', '/path')).toBe('zip');
    expect(extractExtension(null, '/path/document.PDF')).toBe('pdf');
    expect(extractExtension(null, '/path/no_extension')).toBe(null);
  });

  it('returns excluded-host when URL host is on exclusion list', () => {
    const outcome = evaluateCapture(basePolicy, 'https://bank.example.test/statement.pdf');
    expect(outcome.decision).toBe('excluded-host');
    expect(outcome.reason).toContain('bank.example.test');
  });

  it('returns excluded-address when URL or path is on exclusion list', () => {
    const outcome1 = evaluateCapture(basePolicy, 'https://example.test/private/doc.pdf');
    expect(outcome1.decision).toBe('excluded-address');

    const outcome2 = evaluateCapture(basePolicy, 'https://other.test/secret/data.zip');
    expect(outcome2.decision).toBe('excluded-address');
  });

  it('returns allowed when file extension is in inclusion list', () => {
    const outcome = evaluateCapture(basePolicy, 'https://public.example.test/archive.zip');
    expect(outcome.decision).toBe('allowed');
  });

  it('handles confirmationMode: alwaysAsk', () => {
    const policy = { ...basePolicy, confirmationMode: 'alwaysAsk' };
    const outcome = evaluateCapture(policy, 'https://public.example.test/archive.zip');
    expect(outcome.decision).toBe('confirmation-required');
  });

  it('handles confirmationMode: askForUnknown with unknown extension', () => {
    const policy = { ...basePolicy, confirmationMode: 'askForUnknown' };
    const outcome = evaluateCapture(policy, 'https://public.example.test/file.unknown');
    expect(outcome.decision).toBe('confirmation-required');
  });

  it('handles confirmationMode: automatic with unknown extension', () => {
    const policy = { ...basePolicy, confirmationMode: 'automatic' };
    const outcome = evaluateCapture(policy, 'https://public.example.test/file.unknown');
    expect(outcome.decision).toBe('type-not-enabled');
  });

  it('returns type-not-enabled on malformed URL', () => {
    const outcome = evaluateCapture(basePolicy, 'not-a-valid-url');
    expect(outcome.decision).toBe('type-not-enabled');
  });

  it('returns type-not-enabled when policy is disabled', () => {
    const policy = { ...basePolicy, enabled: false };
    const outcome = evaluateCapture(policy, 'https://example.test/archive.zip');
    expect(outcome.decision).toBe('type-not-enabled');
  });
});
