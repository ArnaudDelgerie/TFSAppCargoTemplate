import './styles/app.css';
import { initDemo } from './demo';

// Plain webpack-bundled vanilla JS — no frontend framework is imposed by the
// template. Swap in Stimulus, Turbo, React… on your terms; the demo just needs
// fetch + EventSource.
document.addEventListener('DOMContentLoaded', initDemo);
