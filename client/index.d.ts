export interface RevealOptions {
  endpoint: string;
  site: string;
  signal?: AbortSignal;
}

export class KnockError extends Error {
  constructor(code: string, status?: number);
  code: string;
  status: number;
}

export function revealEmail(options: RevealOptions): Promise<string>;
