/**
 * URL prefix when the site is served from a sub-path (e.g. GitHub Pages at
 * /kestrel). Empty for local development and root deployments.
 */
export const BASE_PATH: string = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
