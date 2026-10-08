import './home.css'
import './productDemo.css'
import { initProductDemo } from './productDemo.js'

const demo = document.querySelector('[data-product-demo]')
const dispose = demo ? initProductDemo(demo) : () => {}
if (import.meta.hot) import.meta.hot.dispose(dispose)
