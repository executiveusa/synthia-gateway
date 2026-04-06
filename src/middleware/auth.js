'use strict';

/**
 * Optional gateway-level authentication middleware.
 *
 * Behaviour:
 *  - If GATEWAY_API_KEY env var is set, every request must include:
 *      Authorization: Bearer <GATEWAY_API_KEY>
 *    In this mode, the actual provider API keys are read from environment
 *    variables (OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.).
 *  - If GATEWAY_API_KEY is NOT set, the gateway operates in pass-through mode:
 *    the caller's Authorization header is forwarded directly to the provider,
 *    so the caller supplies their own provider key.
 */

function auth(req, res, next) {
  const gatewayKey = process.env.GATEWAY_API_KEY;

  if (!gatewayKey) {
    // Pass-through mode — no gateway-level auth required.
    return next();
  }

  const authHeader = req.headers['authorization'] || '';
  const token = authHeader.startsWith('Bearer ')
    ? authHeader.slice('Bearer '.length).trim()
    : null;

  if (!token || token !== gatewayKey) {
    return res.status(401).json({
      error: {
        message: 'Unauthorized: invalid or missing gateway API key.',
        type: 'authentication_error',
        code: 'invalid_api_key',
      },
    });
  }

  next();
}

module.exports = auth;
