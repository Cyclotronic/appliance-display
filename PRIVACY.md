# Privacy

`appliance-display` contains no telemetry, analytics, crash uploader, update
service, or maintainer-operated network endpoint.

The daemon communicates only through a local Unix socket and the configured
local serial device. Display text is held in memory long enough to send it to
the panel. The current release does not persist displayed text or button events.

Normal service logs may contain operational errors and device paths. Review
those logs before attaching them to a public issue if local paths or host names
are sensitive in your environment.
