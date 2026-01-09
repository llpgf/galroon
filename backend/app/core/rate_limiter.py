"""
Rate Limiter - Phase 19.8: API Security & Performance

Security hardening to prevent API abuse and DoS attacks.
Uses slowapi for IP-based rate limiting.

Created as separate module to avoid circular imports.
"""

from slowapi import Limiter
from slowapi.util import get_remote_address

from .config import settings

# Initialize rate limiter
# key_func=get_remote_address means counting by client IP
rate_limits_env = settings.GALGAME_RATE_LIMITS
if rate_limits_env:
    default_limits = [limit.strip() for limit in rate_limits_env.split(",") if limit.strip()]
else:
    default_limits = ["100/minute"]

limiter = Limiter(key_func=get_remote_address, default_limits=default_limits)
