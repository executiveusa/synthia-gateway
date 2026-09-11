-- Register the Groq and Cloudflare lanes in the provider health table.

INSERT OR IGNORE INTO provider_status (provider) VALUES
    ('groq'),
    ('cloudflare');
