import type { Config } from 'tailwindcss'

export default {
  darkMode: ['class'],
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        display: ['"Space Grotesk"', 'ui-sans-serif', 'system-ui'],
        body: ['"IBM Plex Sans"', 'ui-sans-serif', 'system-ui'],
      },
      colors: {
        canvas: '#f6f2ea',
        ink: '#121212',
        accent: '#0b5b5f',
        accentGlow: '#64c3b7',
        surface: '#ffffff',
        muted: '#6f6a5e',
        stroke: '#e3d8c7',
        warning: '#d1752d',
        ok: '#2b8a5a',
        danger: '#b63b2e',
      },
      boxShadow: {
        soft: '0 18px 45px rgba(15, 24, 39, 0.12)',
      },
      backgroundImage: {
        'paper-gradient':
          'radial-gradient(circle at 15% 10%, #fff9f1 0%, #f6f2ea 45%, #efe4d7 100%)',
      },
    },
  },
  plugins: [],
} satisfies Config
