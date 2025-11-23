/**
 * Interactive Graph Visualization
 * Adapted from Quartz's D3 + PixiJS force-directed graph
 */

import {
  SimulationNodeDatum,
  SimulationLinkDatum,
  Simulation,
  forceSimulation,
  forceManyBody,
  forceCenter,
  forceLink,
  forceCollide,
  zoomIdentity,
  select,
  drag,
  zoom,
} from 'd3';
import { Text, Graphics, Application, Container, Circle } from 'pixi.js';
import { Group as TweenGroup, Tween as Tweened } from '@tweenjs/tween.js';

export interface D3Config {
  drag: boolean;
  zoom: boolean;
  depth: number;
  scale: number;
  repelForce: number;
  centerForce: number;
  linkDistance: number;
  fontSize: number;
  opacityScale: number;
  focusOnHover: boolean;
}

type GraphicsInfo = {
  color: string;
  gfx: Graphics;
  alpha: number;
  active: boolean;
};

type NodeData = {
  id: string;
  text: string;
  tags: string[];
} & SimulationNodeDatum;

type SimpleLinkData = {
  source: string;
  target: string;
};

type LinkData = {
  source: NodeData;
  target: NodeData;
} & SimulationLinkDatum<NodeData>;

type LinkRenderData = GraphicsInfo & {
  simulationData: LinkData;
};

type NodeRenderData = GraphicsInfo & {
  simulationData: NodeData;
  label: Text;
};

const localStorageKey = 'graph-visited';

function getVisited(): Set<string> {
  return new Set(JSON.parse(localStorage.getItem(localStorageKey) ?? '[]'));
}

export function addToVisited(slug: string) {
  const visited = getVisited();
  visited.add(slug);
  localStorage.setItem(localStorageKey, JSON.stringify([...visited]));
}

type TweenNode = {
  update: (time: number) => void;
  stop: () => void;
};

export async function renderGraph(
  container: HTMLElement,
  currentSlug: string,
  graphData: { nodes: any[]; edges: any[] },
  config: D3Config,
): Promise<() => void> {
  const visited = getVisited();
  container.innerHTML = '';

  const {
    drag: enableDrag,
    zoom: enableZoom,
    depth,
    scale,
    repelForce,
    centerForce,
    linkDistance,
    fontSize,
    opacityScale,
    focusOnHover,
  } = config;

  // Build node/link data structures
  const links: SimpleLinkData[] = graphData.edges.map((e) => ({
    source: e.source,
    target: e.target,
  }));

  const validNodes = new Set(graphData.nodes.map((n) => n.id));
  const neighbourhood = new Set<string>();

  // Compute neighborhood based on depth
  const wl: (string | '__SENTINEL')[] = [currentSlug, '__SENTINEL'];
  let currentDepth = depth;

  if (depth >= 0) {
    while (currentDepth >= 0 && wl.length > 0) {
      const cur = wl.shift()!;
      if (cur === '__SENTINEL') {
        currentDepth--;
        if (currentDepth >= 0) wl.push('__SENTINEL');
      } else {
        neighbourhood.add(cur);
        const outgoing = links.filter((l) => l.source === cur);
        const incoming = links.filter((l) => l.target === cur);
        wl.push(...outgoing.map((l) => l.target), ...incoming.map((l) => l.source));
      }
    }
  } else {
    validNodes.forEach((id) => neighbourhood.add(id));
  }

  const nodes: NodeData[] = [...neighbourhood]
    .filter((id) => validNodes.has(id))
    .map((id) => {
      const node = graphData.nodes.find((n) => n.id === id);
      return {
        id,
        text: node?.title || id,
        tags: node?.tags || [],
      };
    });

  const filteredLinks: LinkData[] = links
    .filter((l) => neighbourhood.has(l.source) && neighbourhood.has(l.target))
    .map((l) => ({
      source: nodes.find((n) => n.id === l.source),
      target: nodes.find((n) => n.id === l.target),
    }))
    .filter((l): l is LinkData => l.source !== undefined && l.target !== undefined);

  const width = container.offsetWidth;
  const height = Math.max(container.offsetHeight, 250);

  // D3 force simulation
  const simulation: Simulation<NodeData, LinkData> = forceSimulation<NodeData>(nodes)
    .force('charge', forceManyBody().strength(-100 * repelForce))
    .force('center', forceCenter().strength(centerForce))
    .force('link', forceLink(filteredLinks).distance(linkDistance))
    .force(
      'collide',
      forceCollide<NodeData>((n) => nodeRadius(n)).iterations(3),
    );

  // CSS variables for colors
  const cssVars = ['--text-color', '--text-color-alt', '--accent-color', '--border-color'] as const;
  const computedStyleMap = cssVars.reduce(
    (acc, key) => {
      acc[key] = getComputedStyle(document.documentElement).getPropertyValue(key);
      return acc;
    },
    {} as Record<(typeof cssVars)[number], string>,
  );

  const color = (d: NodeData) => {
    const isCurrent = d.id === currentSlug;
    if (isCurrent) {
      return computedStyleMap['--accent-color'] || '#0066cc';
    } else if (visited.has(d.id)) {
      return computedStyleMap['--text-color-alt'] || '#666';
    } else {
      return computedStyleMap['--border-color'] || '#d0d0d0';
    }
  };

  function nodeRadius(d: NodeData) {
    const numLinks = filteredLinks.filter((l) => l.source.id === d.id || l.target.id === d.id).length;
    return 2 + Math.sqrt(numLinks);
  }

  let hoveredNodeId: string | null = null;
  let hoveredNeighbours: Set<string> = new Set();
  const linkRenderData: LinkRenderData[] = [];
  const nodeRenderData: NodeRenderData[] = [];

  function updateHoverInfo(newHoveredId: string | null) {
    hoveredNodeId = newHoveredId;

    if (newHoveredId === null) {
      hoveredNeighbours = new Set();
      for (const n of nodeRenderData) {
        n.active = false;
      }
      for (const l of linkRenderData) {
        l.active = false;
      }
    } else {
      hoveredNeighbours = new Set();
      for (const l of linkRenderData) {
        const linkData = l.simulationData;
        if (linkData.source.id === newHoveredId || linkData.target.id === newHoveredId) {
          hoveredNeighbours.add(linkData.source.id);
          hoveredNeighbours.add(linkData.target.id);
        }
        l.active = linkData.source.id === newHoveredId || linkData.target.id === newHoveredId;
      }

      for (const n of nodeRenderData) {
        n.active = hoveredNeighbours.has(n.simulationData.id);
      }
    }
  }

  const tweens = new Map<string, TweenNode>();
  let dragStartTime = 0;
  let dragging = false;

  function renderLinks() {
    tweens.get('link')?.stop();
    const tweenGroup = new TweenGroup();

    for (const l of linkRenderData) {
      let alpha = 1;
      if (hoveredNodeId) {
        alpha = l.active ? 1 : 0.2;
      }
      l.color = l.active ? (computedStyleMap['--text-color'] || '#1a1a1a') : (computedStyleMap['--border-color'] || '#d0d0d0');
      tweenGroup.add(new Tweened<LinkRenderData>(l).to({ alpha }, 200));
    }

    tweenGroup.getAll().forEach((tw) => tw.start());
    tweens.set('link', {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw) => tw.stop());
      },
    });
  }

  function renderLabels() {
    tweens.get('label')?.stop();
    const tweenGroup = new TweenGroup();

    const defaultScale = 1 / scale;
    const activeScale = defaultScale * 1.1;
    for (const n of nodeRenderData) {
      const nodeId = n.simulationData.id;

      if (hoveredNodeId === nodeId) {
        tweenGroup.add(
          new Tweened<Text>(n.label).to(
            {
              alpha: 1,
              scale: { x: activeScale, y: activeScale },
            },
            100,
          ),
        );
      } else {
        tweenGroup.add(
          new Tweened<Text>(n.label).to(
            {
              alpha: n.label.alpha,
              scale: { x: defaultScale, y: defaultScale },
            },
            100,
          ),
        );
      }
    }

    tweenGroup.getAll().forEach((tw) => tw.start());
    tweens.set('label', {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw) => tw.stop());
      },
    });
  }

  function renderNodes() {
    tweens.get('hover')?.stop();

    const tweenGroup = new TweenGroup();
    for (const n of nodeRenderData) {
      let alpha = 1;

      if (hoveredNodeId !== null && focusOnHover) {
        alpha = n.active ? 1 : 0.2;
      }

      tweenGroup.add(new Tweened<Graphics>(n.gfx, tweenGroup).to({ alpha }, 200));
    }

    tweenGroup.getAll().forEach((tw) => tw.start());
    tweens.set('hover', {
      update: tweenGroup.update.bind(tweenGroup),
      stop() {
        tweenGroup.getAll().forEach((tw) => tw.stop());
      },
    });
  }

  function renderPixiFromD3() {
    renderNodes();
    renderLinks();
    renderLabels();
  }

  tweens.forEach((tween) => tween.stop());
  tweens.clear();

  const app = new Application();
  await app.init({
    width,
    height,
    antialias: true,
    autoStart: false,
    autoDensity: true,
    backgroundAlpha: 0,
    preference: 'webgpu',
    resolution: window.devicePixelRatio,
    eventMode: 'static',
  });
  container.appendChild(app.canvas);

  const stage = app.stage;
  stage.interactive = false;

  const labelsContainer = new Container<Text>({ zIndex: 3, isRenderGroup: true });
  const nodesContainer = new Container<Graphics>({ zIndex: 2, isRenderGroup: true });
  const linkContainer = new Container<Graphics>({ zIndex: 1, isRenderGroup: true });
  stage.addChild(nodesContainer, labelsContainer, linkContainer);

  for (const n of nodes) {
    const nodeId = n.id;

    const label = new Text({
      interactive: false,
      eventMode: 'none',
      text: n.text,
      alpha: 0,
      anchor: { x: 0.5, y: 1.2 },
      style: {
        fontSize: fontSize * 15,
        fill: computedStyleMap['--text-color'] || '#1a1a1a',
        fontFamily: 'Berkeley Mono, monospace',
      },
      resolution: window.devicePixelRatio * 4,
    });
    label.scale.set(1 / scale);

    let oldLabelOpacity = 0;
    const gfx = new Graphics({
      interactive: true,
      label: nodeId,
      eventMode: 'static',
      hitArea: new Circle(0, 0, nodeRadius(n)),
      cursor: 'pointer',
    })
      .circle(0, 0, nodeRadius(n))
      .fill({ color: color(n) })
      .on('pointerover', (e) => {
        updateHoverInfo(e.target.label);
        oldLabelOpacity = label.alpha;
        if (!dragging) {
          renderPixiFromD3();
        }
      })
      .on('pointerleave', () => {
        updateHoverInfo(null);
        label.alpha = oldLabelOpacity;
        if (!dragging) {
          renderPixiFromD3();
        }
      });

    nodesContainer.addChild(gfx);
    labelsContainer.addChild(label);

    const nodeRenderDatum: NodeRenderData = {
      simulationData: n,
      gfx,
      label,
      color: color(n),
      alpha: 1,
      active: false,
    };

    nodeRenderData.push(nodeRenderDatum);
  }

  for (const l of filteredLinks) {
    const gfx = new Graphics({ interactive: false, eventMode: 'none' });
    linkContainer.addChild(gfx);

    const linkRenderDatum: LinkRenderData = {
      simulationData: l,
      gfx,
      color: computedStyleMap['--border-color'] || '#d0d0d0',
      alpha: 1,
      active: false,
    };

    linkRenderData.push(linkRenderDatum);
  }

  let currentTransform = zoomIdentity;
  if (enableDrag) {
    select<HTMLCanvasElement, NodeData | undefined>(app.canvas).call(
      drag<HTMLCanvasElement, NodeData | undefined>()
        .container(() => app.canvas)
        .subject(() => nodes.find((n) => n.id === hoveredNodeId))
        .on('start', function dragstarted(event) {
          if (!event.active) simulation.alphaTarget(1).restart();
          event.subject.fx = event.subject.x;
          event.subject.fy = event.subject.y;
          (event.subject as any).__initialDragPos = {
            x: event.subject.x,
            y: event.subject.y,
            fx: event.subject.fx,
            fy: event.subject.fy,
          };
          dragStartTime = Date.now();
          dragging = true;
        })
        .on('drag', function dragged(event) {
          const initPos = (event.subject as any).__initialDragPos;
          event.subject.fx = initPos.x + (event.x - initPos.x) / currentTransform.k;
          event.subject.fy = initPos.y + (event.y - initPos.y) / currentTransform.k;
        })
        .on('end', function dragended(event) {
          if (!event.active) simulation.alphaTarget(0);
          event.subject.fx = null;
          event.subject.fy = null;
          dragging = false;

          if (Date.now() - dragStartTime < 500) {
            const node = nodes.find((n) => n.id === event.subject.id) as NodeData;
            window.location.href = `/${node.id}.html`;
          }
        }),
    );
  } else {
    for (const node of nodeRenderData) {
      node.gfx.on('click', () => {
        window.location.href = `/${node.simulationData.id}.html`;
      });
    }
  }

  if (enableZoom) {
    select<HTMLCanvasElement, NodeData>(app.canvas).call(
      zoom<HTMLCanvasElement, NodeData>()
        .extent([
          [0, 0],
          [width, height],
        ])
        .scaleExtent([0.25, 4])
        .on('zoom', ({ transform }) => {
          currentTransform = transform;
          stage.scale.set(transform.k, transform.k);
          stage.position.set(transform.x, transform.y);

          const scale = transform.k * opacityScale;
          let scaleOpacity = Math.max((scale - 1) / 3.75, 0);
          const activeNodes = nodeRenderData.filter((n) => n.active).flatMap((n) => n.label);

          for (const label of labelsContainer.children) {
            if (!activeNodes.includes(label)) {
              label.alpha = scaleOpacity;
            }
          }
        }),
    );
  }

  let stopAnimation = false;
  function animate(time: number) {
    if (stopAnimation) return;
    for (const n of nodeRenderData) {
      const { x, y } = n.simulationData;
      if (!x || !y) continue;
      n.gfx.position.set(x + width / 2, y + height / 2);
      if (n.label) {
        n.label.position.set(x + width / 2, y + height / 2);
      }
    }

    for (const l of linkRenderData) {
      const linkData = l.simulationData;
      l.gfx.clear();
      l.gfx.moveTo(linkData.source.x! + width / 2, linkData.source.y! + height / 2);
      l.gfx
        .lineTo(linkData.target.x! + width / 2, linkData.target.y! + height / 2)
        .stroke({ alpha: l.alpha, width: 1, color: l.color });
    }

    tweens.forEach((t) => t.update(time));
    app.renderer.render(stage);
    requestAnimationFrame(animate);
  }

  requestAnimationFrame(animate);

  return () => {
    stopAnimation = true;
    app.destroy();
  };
}
