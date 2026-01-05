"""
Rate Limiter - Phase 19.8: API Security & Performance

Security hardening to prevent API abuse and DoS attacks.
Uses slowapi for IP-based rate limiting.

Created as separate module to avoid circular imports.
"""

from slowapi import Limiter
from slowapi.util import get_remote_address

# Initialize rate limiter
# key_func=get_remote_address means counting by client IP
limiter = Limiter(key_func=get_remote_address)
