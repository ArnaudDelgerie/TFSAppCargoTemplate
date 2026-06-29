import './styles/app.css';
import '@hotwired/turbo';
import { Application } from '@hotwired/stimulus';
import DemoController from './controllers/demo_controller';

window.Stimulus = Application.start();
window.Stimulus.register('demo', DemoController);
