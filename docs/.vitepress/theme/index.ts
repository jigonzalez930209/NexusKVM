import DefaultTheme from 'vitepress/theme';
import { h } from 'vue';
import AnimatedSvgBackground from './components/AnimatedSvgBackground.vue';
import RkvmCitation from './components/RkvmCitation.vue';
import TopologyDiagram from './components/TopologyDiagram.vue';
import PermissionMatrix from './components/PermissionMatrix.vue';
import './style.css';

export default {
  extends: DefaultTheme,
  Layout() {
    return h(DefaultTheme.Layout, null, {
      'layout-top': () => h(AnimatedSvgBackground),
    });
  },
  enhanceApp({ app }) {
    app.component('AnimatedSvgBackground', AnimatedSvgBackground);
    app.component('RkvmCitation', RkvmCitation);
    app.component('TopologyDiagram', TopologyDiagram);
    app.component('PermissionMatrix', PermissionMatrix);
  },
};
