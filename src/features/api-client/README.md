# API client

This feature owns the reusable REST and GraphQL request composer used by the
runtime console and the focused API trace workbench.

- `model.ts` contains pure draft, request-building, endpoint-target, and body
  parsing helpers.
- `useApiClientSession.ts` owns one request lifecycle. It publishes the exact
  outbound request before awaiting its response so debugger data views can
  describe an in-flight request without guessing.
- `ApiClientPane.tsx` renders the existing API composer and optional full
  response presentation without owning transport state.

Network policy and redaction remain in the native HTTP bridge. This feature
only prepares typed requests and presents the response returned to the UI.
