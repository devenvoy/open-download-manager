// Created: 2026-11-10
// Reason: test(formatters): comprehensive test suite for modularized data formatters
// Description: Unit tests verifying numeric conversion, redaction, status/category mapping, byte formatting, and duration calculations.

'use strict';

const test = require('node:test');
const assert = require('node:assert');
const {
	numeric,
	redactUrl,
	statusOf,
	categoryOf,
	bytes,
	date,
	duration,
	timeLeft,
	sizeLabel,
	percentage,
	messageOf,
	formatProxyDisplay,
} = require('./formatters.js');

test('numeric converts valid positive numbers and handles invalid values', () => {
	assert.equal(numeric(123), 123);
	assert.equal(numeric('456.78'), 456.78);
	assert.equal(numeric(0), 0);
	assert.equal(numeric(-50), 0);
	assert.equal(numeric(null), 0);
	assert.equal(numeric(undefined), 0);
	assert.equal(numeric('abc'), 0);
	assert.equal(numeric(NaN), 0);
	assert.equal(numeric(Infinity), 0);
});

test('redactUrl strips sensitive credentials and query parameters', () => {
	assert.equal(redactUrl(''), '');
	assert.equal(redactUrl(null), '');

	// HTTP URL
	assert.equal(
		redactUrl('https://user:pass@example.com:8443/file.zip?token=secret#hash'),
		'https://example.com:8443/file.zip'
	);

	// FTP URL with credentials
	assert.equal(
		redactUrl('ftp://alice:secret123@files.example.org/pub/archive.tar.gz'),
		'ftp://******:******@files.example.org/pub/archive.tar.gz'
	);

	// FTP URL with non-standard port
	assert.equal(
		redactUrl('ftp://alice:secret123@files.example.org:2121/pub/archive.tar.gz'),
		'ftp://******:******@files.example.org:2121/pub/archive.tar.gz'
	);

	// Malformed URL fallback
	assert.equal(redactUrl('not-a-valid-url'), 'not-a-valid-url');
});

test('statusOf normalizes raw status variants', () => {
	assert.equal(statusOf({ status: 'active' }), 'downloading');
	assert.equal(statusOf({ status: 'running' }), 'downloading');
	assert.equal(statusOf({ status: 'probing' }), 'connecting');
	assert.equal(statusOf({ status: 'complete' }), 'completed');
	assert.equal(statusOf({ status: 'done' }), 'completed');
	assert.equal(statusOf({ status: 'error' }), 'failed');
	assert.equal(statusOf({ status: 'canceled' }), 'cancelled');
	assert.equal(statusOf({ status: 'pending' }), 'queued');
	assert.equal(statusOf({ status: 'paused' }), 'paused');
	assert.equal(statusOf({}), 'queued');
});

test('categoryOf classifies downloads into display categories', () => {
	assert.equal(categoryOf({ category: 'video/mp4' }), 'Video');
	assert.equal(categoryOf({ category: 'AUDIO' }), 'Audio');
	assert.equal(categoryOf({ category: 'music/flac' }), 'Audio');
	assert.equal(categoryOf({ category: 'documents' }), 'Documents');
	assert.equal(categoryOf({ category: 'text/plain' }), 'Documents');
	assert.equal(categoryOf({ category: 'compressed-archive' }), 'Archives');
	assert.equal(categoryOf({ category: 'programs' }), 'Programs');
	assert.equal(categoryOf({ category: 'random' }), 'Other');
	assert.equal(categoryOf({}), 'Other');
});

test('bytes formats sizes across unit scales accurately', () => {
	assert.equal(bytes(0), '0 B');
	assert.equal(bytes(-10), '0 B');
	assert.equal(bytes(500), '500 B');
	assert.equal(bytes(1024), '1.00 KB');
	assert.equal(bytes(1536), '1.50 KB');
	assert.equal(bytes(1048576), '1.00 MB');
	assert.equal(bytes(1073741824), '1.00 GB');
	assert.equal(bytes(1099511627776), '1.00 TB');
});

test('duration formats seconds into human units', () => {
	assert.equal(duration(25), '25 sec');
	assert.equal(duration(59), '59 sec');
	assert.equal(duration(60), '1 min');
	assert.equal(duration(125), '3 min');
	assert.equal(duration(3600), '1 hr 0 min');
	assert.equal(duration(3665), '1 hr 1 min');
});

test('timeLeft calculates remaining time from total and speed', () => {
	assert.equal(timeLeft(null), '');
	assert.equal(timeLeft({ total: 1000, downloaded: 500, speed: 0 }), '');
	assert.equal(timeLeft({ total: 1000, downloaded: 500, speed: 250 }), '2 sec');
	assert.equal(timeLeft({ total: 1000, downloaded: 500, speed: 250, estimated: true }), '≈ 2 sec');
});

test('percentage computes accurate progress values', () => {
	assert.equal(percentage(null), null);
	assert.equal(percentage({ status: 'completed' }), 100);
	assert.equal(percentage({ total: 1000, downloaded: 250 }), 25);
	assert.equal(percentage({ total: 1000, downloaded: 2000 }), 100);
	assert.equal(percentage({ streamProgress: { completed: 5, total: 10 } }), 50);
	assert.equal(percentage({}), null);
});

test('messageOf extracts human error message', () => {
	assert.equal(messageOf(new Error('Network timeout')), 'Network timeout');
	assert.equal(messageOf({ message: 'Disk full' }), 'Disk full');
	assert.equal(messageOf('Direct string error'), 'Direct string error');
	assert.equal(messageOf(null), 'The operation failed.');
});

test('formatProxyDisplay formats proxy server address', () => {
	assert.equal(formatProxyDisplay(null), 'Direct (No Proxy)');
	assert.equal(formatProxyDisplay({}), 'Direct (No Proxy)');
	assert.equal(formatProxyDisplay({ enabled: false }), 'Direct (No Proxy)');
	assert.equal(formatProxyDisplay({ enabled: true, protocol: 'direct' }), 'Direct (No Proxy)');
	assert.equal(
		formatProxyDisplay({ enabled: true, host: '127.0.0.1', port: 8080, protocol: 'http' }),
		'HTTP 127.0.0.1:8080'
	);
	assert.equal(
		formatProxyDisplay({ enabled: true, host: 'proxy.corp.local', port: 1080, protocol: 'socks5', username: 'admin' }),
		'SOCKS5 admin@proxy.corp.local:1080'
	);
});
