// Static-asset front door. `about.phonia.app` is the marketing subdomain:
// its root document is the landing page, not the app shell, and needs the
// landing page's own prerendered HTML (title, description, canonical, OG
// tags) rather than the generic app-shell document that every other path
// serves. Static asset routing has no per-hostname rule, so a request to
// the asset store for "/" is host-blind; this worker is the one place that
// can see the Host header before the asset lookup happens, and rewrites
// only that one path on that one host to the prerendered `/landing` file.
// Every other request — including the assets that landing.html itself
// requests once it's on the page — passes straight through unchanged.
interface Env {
  ASSETS: { fetch(request: Request): Promise<Response> };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (request.headers.get('host') === 'about.phonia.app' && url.pathname === '/') {
      url.pathname = '/landing';
    }
    return env.ASSETS.fetch(new Request(url, request));
  }
};
