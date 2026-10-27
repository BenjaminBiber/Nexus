// Fehler einer Server-Antwort mit Code, deutscher Meldung und Status; bei Konflikten der aktuelle Stand.
export class ApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
    readonly current?: unknown,
    readonly details?: Record<string, unknown>,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}
