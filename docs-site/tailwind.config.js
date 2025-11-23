import typography from '@tailwindcss/typography';

/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Warm, easy-on-the-eyes color palette
        sage: {
          50: '#f0f4f1',
          100: '#e8f0e9',
          200: '#d4e4d6',
          300: '#b3d0b7',
          400: '#8bb392',
          500: '#5a6c5d',
          600: '#4a5a4c',
          700: '#3d4a3e',
          800: '#333d34',
          900: '#2d342e',
        },
      },
    },
  },
  plugins: [
    typography,
  ],
}

