# knock

An email reveal API built with Rust and Cloudflare Workers. Deploy your own
Worker, configure your websites, and add the browser client to each site. You
control the UI. Signed proof-of-work challenges, rate limits, and SQLite Durable
Objects control access and prevent replay.

## Setup

Fork and clone this repository. On macOS or Linux, install Deno 2.9.8 and just
1.59.0, then run `just setup` to install the pinned Rust toolchain and
dependencies. Linux builds need a C toolchain, `pkg-config`, and OpenSSL
development headers; macOS needs Xcode Command Line Tools.

Tool sources are checksum-verified and installed with the lockfiles in `tools/`.
The installer removes an unused `binary-install` dependency from `worker-build`.
`just audit` checks application and tool dependencies.

Copy [.dev.vars.example](.dev.vars.example) to `.dev.vars`. Replace
`YOUR_GENERATED_KEY` with the output of
`openssl rand -base64 32 | tr '+/' '-_' | tr -d '=\n'`. Keep secrets out of
version control.

Run `just dev` to serve at `http://127.0.0.1:8787`. Use `just check`,
`just fmt`, and `just audit` for maintenance. Run `just test-integration`
against the local server; wait until the next minute before repeating rate-limit
tests.

## Usage

Copy `client/index.js` and `client/solver.js` into your site's public `knock/`
directory and retain [LICENSE](LICENSE). TypeScript projects can also copy
`client/index.d.ts`.

```js
import { revealEmail } from "/knock/index.js";

const email = await revealEmail({
  endpoint: "https://knock.example.com",
  site: "example",
});
```

The client returns the address, accepts an optional abort `signal`, and solves
challenges in a Web Worker. Allow local scripts and workers in CSP and the API
origin in `connect-src`. Handle rejected promises in your UI and provide another
contact route for visitors without JavaScript.

The API accepts JSON with an approved `Origin` header.

- `POST /v1/challenge` accepts `{ "site": "example" }` and returns a token,
  difficulty, expiration, version, and algorithm.
- `POST /v1/reveal` accepts `site`, `token`, and `nonce`, then returns `email`.

Version 1 uses SHA-256 of UTF-8 `token + "." + nonce`, requiring `difficulty`
leading zero bits. The nonce is a decimal unsigned 32-bit integer string without
leading zeros. Challenges last 120 seconds and permit one redemption from the
same site, origin, and IP. Errors return `{ "error": "code" }`.

## Deployment

Connect the repository to Cloudflare Workers Builds.

| Setting        | Value                              |
| -------------- | ---------------------------------- |
| Worker         | `knock`                            |
| Root           | `/`                                |
| Build Command  | `sh scripts/cloudflare.sh build`   |
| Deploy Command | `sh scripts/cloudflare.sh publish` |
| Environment    | `SKIP_DEPENDENCY_INSTALL=true`     |
| Preview Builds | Disabled                           |

Set `SITES` and `SIGNING_KEY` as Worker secrets, add your API custom domain, and
use Cloudflare's generated build token. Follow
[config.example.json](config.example.json) for production sites, using exact
HTTPS origins without trailing slashes.

`just deploy-check` runs checks and a deployment dry run. `just deploy`
publishes with `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` from your
environment.

## Limitations

Bots can solve challenges and retain addresses. CORS is not authentication.
Per-minute limits are 10 challenges and 20 reveal attempts per IP, and 300
requests per action per site. Shared networks share limits.

Durable Objects store rotating keyed IP hashes, counters, and redemption IDs.
Counters expire at the next minute boundary; redemption IDs expire with their
challenge. Requests and alarms clean up expired rows. Cloudflare's retention and
backups are separate. No cookies, analytics, or browser storage are used.

## Contributing

Open an issue or pull request. Run `just check` and `just test-integration` for
behavior changes. Keep dependency versions pinned and update their lockfiles.

## License

[MIT](LICENSE).
