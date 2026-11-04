// A machine-readable index for agents and scripts: the latest release and
// its downloads, checksums and attestation, how to install and verify, what
// each verdict and exit code means, the indicator files and the key pages.
// Static, built with the site. Described in llms.txt and on the About page.
import type { APIRoute } from 'astro';
import { facts } from '../../machine';

export const GET: APIRoute = async () =>
  new Response(JSON.stringify({ schemaVersion: 1, generated: new Date().toISOString().slice(0, 10), ...(await facts()) }, null, 2) + '\n', {
    headers: { 'Content-Type': 'application/json; charset=utf-8' },
  });
