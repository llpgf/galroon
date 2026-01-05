import sys
sys.path.insert(0, '.')

from app.main import app

print('=== FastAPI Routes ===')
print(f'Type: {type(app).__name__}')

# Get all routes from app.routes
routes = list(app.routes)
print(f'Total routes: {len(routes)}')

for i, route in enumerate(routes):
    print(f'{i+1}. Path: {route.path}')
    print(f'   Type: {type(route).__name__}')
    print(f'   Name: {getattr(route, "name", "N/A")}')

print()
print('=== Route Details ===')
for i, route in enumerate(routes[:5]):  # Show first 5 routes only
    print(f'{i+1}. {route.path} ({type(route).__name__})')
    print(f'   Routes: {len(route.routes)}')
    if route.path.startswith('/api/v1'):
        print(f'   Has include_router: {hasattr(route, "include_router")}')
