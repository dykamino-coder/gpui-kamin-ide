import { createApiReference } from '@scalar/api-reference'
import '@scalar/api-reference/style.css'

createApiReference('#api-reference', {
  url: '/api/openapi.json',
  withDefaultFonts: false,
  hideClientButton: true,
  persistAuth: false,
  customFetch: (input, init) => {
    const request = new Request(typeof input === 'string' ? new URL(input, location.href) : input, init)
    const url = new URL(request.url, location.href)
    const token = localStorage.getItem('dashboard_session_token')
    if (url.origin === location.origin && url.pathname === '/api/openapi.json' && token) {
      request.headers.set('Authorization', `Bearer ${token}`)
    }
    return fetch(request)
  },
  authentication: {
    preferredSecurityScheme: 'adminBearer',
    securitySchemes: {
      adminBearer: { token: localStorage.getItem('dashboard_session_token') ?? '' },
    },
  },
})
