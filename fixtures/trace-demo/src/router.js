export function createRouter() {
  const routes = [];

  return {
    get(path, handler) {
      routes.push({ method: "GET", path, handler });
    },
    async dispatch(request, response) {
      const url = new URL(request.url, "http://localhost");
      const route = routes.find(
        (candidate) =>
          candidate.method === request.method && matchPath(candidate.path, url.pathname),
      );

      if (!route) {
        response.writeHead(404, { "content-type": "application/json" });
        response.end(JSON.stringify({ error: "Route not found" }));
        return;
      }

      const params = extractParams(route.path, url.pathname);
      await route.handler({ request, params }, response);
    },
  };
}

function matchPath(pattern, pathname) {
  const patternParts = pattern.split("/").filter(Boolean);
  const pathParts = pathname.split("/").filter(Boolean);
  return (
    patternParts.length === pathParts.length &&
    patternParts.every((part, index) => part.startsWith(":") || part === pathParts[index])
  );
}

function extractParams(pattern, pathname) {
  const patternParts = pattern.split("/").filter(Boolean);
  const pathParts = pathname.split("/").filter(Boolean);
  return Object.fromEntries(
    patternParts
      .map((part, index) => [part, pathParts[index]])
      .filter(([part]) => part.startsWith(":"))
      .map(([part, value]) => [part.slice(1), value]),
  );
}

