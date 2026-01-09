/**
 * Discovery Lens Component
 * 
 * Interactive WebGL star map with distinct node types:
 * - Game nodes (indigo stars)
 * - Creator nodes (green planets)
 * - Developer nodes (amber cluster cores)
 * 
 * Features: Drag rotation, zoom, hover effects, click-to-focus
 */

import { useEffect, useRef, useCallback, useState } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export type NodeType = 'game' | 'creator' | 'developer';

export interface DiscoveryNode {
      id: string;
      type: NodeType;
      label: string;
      x: number;
      y: number;
      z: number;
      connections: number[];
      size: number;
}

interface DiscoveryLensProps {
      nodes?: DiscoveryNode[];
      onNodeClick?: (node: DiscoveryNode) => void;
      onNodeHover?: (node: DiscoveryNode | null) => void;
}

// Node type colors
const NODE_COLORS: Record<NodeType, THREE.Color> = {
      game: new THREE.Color(0x6366f1),      // Indigo
      creator: new THREE.Color(0x22c55e),   // Green
      developer: new THREE.Color(0xf59e0b)  // Amber
};

// Generate constellation-like data with typed nodes
function generateConstellationData(count: number): DiscoveryNode[] {
      const nodes: DiscoveryNode[] = [];
      const types: NodeType[] = ['game', 'creator', 'developer'];

      for (let i = 0; i < count; i++) {
            // Distribute in a dome-like pattern
            const theta = Math.random() * Math.PI * 2;
            const phi = Math.random() * Math.PI * 0.5;
            const radius = 300 + Math.random() * 200;

            // Assign type with weighted distribution (more games)
            const typeRoll = Math.random();
            const type: NodeType = typeRoll < 0.6 ? 'game' : typeRoll < 0.85 ? 'creator' : 'developer';

            // Size based on type
            const baseSize = type === 'game' ? 3 : type === 'creator' ? 2.5 : 2;
            const size = baseSize + Math.random() * 1.5;

            nodes.push({
                  id: `node-${i}`,
                  type,
                  label: type === 'game' ? `Game ${i}` : type === 'creator' ? `Creator ${i}` : `Developer ${i}`,
                  x: Math.sin(phi) * Math.cos(theta) * radius,
                  y: Math.sin(phi) * Math.sin(theta) * radius,
                  z: Math.cos(phi) * radius - 200,
                  connections: [],
                  size
            });
      }

      // Create constellation connections (prefer same type connections)
      nodes.forEach((node, i) => {
            const connectionCount = Math.floor(Math.random() * 3);
            for (let j = 0; j < connectionCount; j++) {
                  const targetIdx = Math.floor(Math.random() * nodes.length);
                  if (targetIdx !== i && !node.connections.includes(targetIdx)) {
                        node.connections.push(targetIdx);
                  }
            }
      });

      return nodes;
}

function DiscoveryLens({ nodes: externalNodes, onNodeClick, onNodeHover }: DiscoveryLensProps) {
      const containerRef = useRef<HTMLDivElement>(null);
      const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
      const sceneRef = useRef<THREE.Scene | null>(null);
      const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
      const controlsRef = useRef<OrbitControls | null>(null);
      const animationRef = useRef<number>(0);
      const nodesRef = useRef<DiscoveryNode[]>(externalNodes || generateConstellationData(200));
      const timeRef = useRef(0);
      const raycasterRef = useRef(new THREE.Raycaster());
      const mouseRef = useRef(new THREE.Vector2());
      const hoveredNodeRef = useRef<number | null>(null);
      const pointsRef = useRef<THREE.Points | null>(null);
      const sizesRef = useRef<Float32Array | null>(null);
      const originalSizesRef = useRef<Float32Array | null>(null);

      // Focus animation state
      const focusTargetRef = useRef<THREE.Vector3 | null>(null);
      const focusProgressRef = useRef(0);

      const [hoveredNode, setHoveredNode] = useState<DiscoveryNode | null>(null);

      useEffect(() => {
            if (!containerRef.current) return;

            const container = containerRef.current;
            const width = container.clientWidth;
            const height = container.clientHeight;

            // Scene with dark background
            const scene = new THREE.Scene();
            scene.background = new THREE.Color(0x050510);
            scene.fog = new THREE.FogExp2(0x050510, 0.0008);
            sceneRef.current = scene;

            // Camera
            const camera = new THREE.PerspectiveCamera(60, width / height, 1, 2000);
            camera.position.set(0, 100, 500);
            cameraRef.current = camera;

            // Renderer
            const renderer = new THREE.WebGLRenderer({
                  antialias: true,
                  alpha: true
            });
            renderer.setSize(width, height);
            renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
            container.appendChild(renderer.domElement);
            rendererRef.current = renderer;

            // OrbitControls for drag/zoom
            const controls = new OrbitControls(camera, renderer.domElement);
            controls.enableDamping = true;
            controls.dampingFactor = 0.05;
            controls.minDistance = 100;
            controls.maxDistance = 800;
            controls.enablePan = false;
            controls.autoRotate = true;
            controls.autoRotateSpeed = 0.3;
            controlsRef.current = controls;

            // Create star particles with typed colors
            const starGeometry = new THREE.BufferGeometry();
            const starPositions: number[] = [];
            const starSizes: number[] = [];
            const starColors: number[] = [];
            const starTypes: number[] = [];

            nodesRef.current.forEach(node => {
                  starPositions.push(node.x, node.y, node.z);
                  starSizes.push(node.size);
                  const color = NODE_COLORS[node.type];
                  starColors.push(color.r, color.g, color.b);
                  starTypes.push(node.type === 'game' ? 0 : node.type === 'creator' ? 1 : 2);
            });

            starGeometry.setAttribute('position', new THREE.Float32BufferAttribute(starPositions, 3));
            starGeometry.setAttribute('size', new THREE.Float32BufferAttribute(starSizes, 1));
            starGeometry.setAttribute('color', new THREE.Float32BufferAttribute(starColors, 3));
            starGeometry.setAttribute('nodeType', new THREE.Float32BufferAttribute(starTypes, 1));

            // Store references for hover effect
            sizesRef.current = starGeometry.attributes.size.array as Float32Array;
            originalSizesRef.current = new Float32Array(sizesRef.current);

            // Custom shader for typed glowing points
            const starMaterial = new THREE.ShaderMaterial({
                  uniforms: {
                        time: { value: 0 }
                  },
                  vertexShader: `
        attribute float size;
        attribute vec3 color;
        attribute float nodeType;
        varying float vSize;
        varying vec3 vColor;
        varying float vNodeType;
        void main() {
          vSize = size;
          vColor = color;
          vNodeType = nodeType;
          vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
          gl_PointSize = size * (300.0 / -mvPosition.z);
          gl_Position = projectionMatrix * mvPosition;
        }
      `,
                  fragmentShader: `
        uniform float time;
        varying float vSize;
        varying vec3 vColor;
        varying float vNodeType;
        void main() {
          float dist = length(gl_PointCoord - vec2(0.5));
          if (dist > 0.5) discard;
          
          float alpha = smoothstep(0.5, 0.0, dist);
          float glow = pow(alpha, 2.0) * 0.9;
          
          // Type-specific animations
          float pulse;
          if (vNodeType < 0.5) {
            // Game: subtle twinkle
            pulse = 0.85 + 0.15 * sin(time * 1.5 + vSize * 8.0);
          } else if (vNodeType < 1.5) {
            // Creator: gentle pulse
            pulse = 0.7 + 0.3 * sin(time * 2.0 + vSize * 5.0);
          } else {
            // Developer: ring effect
            float ring = smoothstep(0.3, 0.35, dist) * smoothstep(0.45, 0.4, dist);
            glow = glow + ring * 0.5;
            pulse = 0.8 + 0.2 * sin(time * 3.0);
          }
          
          gl_FragColor = vec4(vColor, glow * pulse);
        }
      `,
                  transparent: true,
                  blending: THREE.AdditiveBlending,
                  depthWrite: false
            });

            const stars = new THREE.Points(starGeometry, starMaterial);
            scene.add(stars);
            pointsRef.current = stars;

            // Raycaster threshold for point picking
            raycasterRef.current.params.Points = { threshold: 10 };

            // Create connection lines with type-based colors
            nodesRef.current.forEach(node => {
                  node.connections.forEach(targetIdx => {
                        const target = nodesRef.current[targetIdx];
                        const points = [
                              new THREE.Vector3(node.x, node.y, node.z),
                              new THREE.Vector3(target.x, target.y, target.z)
                        ];
                        const geometry = new THREE.BufferGeometry().setFromPoints(points);

                        // Line color is blend of connected node types
                        const color1 = NODE_COLORS[node.type];
                        const color2 = NODE_COLORS[target.type];
                        const blendedColor = new THREE.Color().lerpColors(color1, color2, 0.5);

                        const lineMaterial = new THREE.LineBasicMaterial({
                              color: blendedColor,
                              transparent: true,
                              opacity: 0.15,
                              blending: THREE.AdditiveBlending
                        });
                        const line = new THREE.Line(geometry, lineMaterial);
                        scene.add(line);
                  });
            });

            // Add grid floor
            const gridHelper = new THREE.GridHelper(1000, 50, 0x111133, 0x111133);
            gridHelper.position.y = -200;
            gridHelper.material.opacity = 0.1;
            gridHelper.material.transparent = true;
            scene.add(gridHelper);

            // Animation loop
            const animate = () => {
                  animationRef.current = requestAnimationFrame(animate);
                  timeRef.current += 0.01;

                  // Update controls
                  controls.update();

                  // Focus animation
                  if (focusTargetRef.current && focusProgressRef.current < 1) {
                        focusProgressRef.current += 0.02;
                        const eased = 1 - Math.pow(1 - focusProgressRef.current, 3);
                        controls.target.lerp(focusTargetRef.current, eased * 0.1);
                        if (focusProgressRef.current >= 1) {
                              controls.target.copy(focusTargetRef.current);
                              focusTargetRef.current = null;
                        }
                  }

                  // Update shader uniforms
                  (starMaterial.uniforms.time as { value: number }).value = timeRef.current;

                  renderer.render(scene, camera);
            };
            animate();

            // Mouse move handler for hover
            const handleMouseMove = (event: MouseEvent) => {
                  const rect = container.getBoundingClientRect();
                  mouseRef.current.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
                  mouseRef.current.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

                  // Raycast
                  if (camera && stars && sizesRef.current && originalSizesRef.current) {
                        raycasterRef.current.setFromCamera(mouseRef.current, camera);
                        const intersects = raycasterRef.current.intersectObject(stars);

                        // Reset previous hovered node size
                        if (hoveredNodeRef.current !== null) {
                              sizesRef.current[hoveredNodeRef.current] = originalSizesRef.current[hoveredNodeRef.current];
                        }

                        if (intersects.length > 0) {
                              const idx = intersects[0].index!;
                              hoveredNodeRef.current = idx;
                              // Scale up hovered node
                              sizesRef.current[idx] = originalSizesRef.current[idx] * 2;
                              starGeometry.attributes.size.needsUpdate = true;

                              const node = nodesRef.current[idx];
                              setHoveredNode(node);
                              onNodeHover?.(node);
                              container.style.cursor = 'pointer';
                        } else {
                              hoveredNodeRef.current = null;
                              starGeometry.attributes.size.needsUpdate = true;
                              setHoveredNode(null);
                              onNodeHover?.(null);
                              container.style.cursor = 'grab';
                        }
                  }
            };

            // Click handler for focus
            const handleClick = (event: MouseEvent) => {
                  if (hoveredNodeRef.current !== null && camera) {
                        const node = nodesRef.current[hoveredNodeRef.current];

                        // Start focus animation
                        focusTargetRef.current = new THREE.Vector3(node.x, node.y, node.z);
                        focusProgressRef.current = 0;

                        // Stop auto-rotate when focusing
                        controls.autoRotate = false;

                        onNodeClick?.(node);
                  }
            };

            container.addEventListener('mousemove', handleMouseMove);
            container.addEventListener('click', handleClick);

            // Handle resize
            const handleResize = () => {
                  if (!container || !renderer || !camera) return;
                  const width = container.clientWidth;
                  const height = container.clientHeight;

                  camera.aspect = width / height;
                  camera.updateProjectionMatrix();
                  renderer.setSize(width, height);
            };
            window.addEventListener('resize', handleResize);

            // Cleanup
            return () => {
                  window.removeEventListener('resize', handleResize);
                  container.removeEventListener('mousemove', handleMouseMove);
                  container.removeEventListener('click', handleClick);
                  cancelAnimationFrame(animationRef.current);
                  controls.dispose();
                  renderer.dispose();
                  if (container.contains(renderer.domElement)) {
                        container.removeChild(renderer.domElement);
                  }
            };
      }, [onNodeClick, onNodeHover]);

      return (
            <div
                  ref={containerRef}
                  className="absolute inset-0 overflow-hidden cursor-grab"
            >
                  {/* Overlay gradient for depth */}
                  <div className="absolute inset-0 bg-gradient-to-b from-transparent via-transparent to-[#0a0a0a]/60 pointer-events-none" />

                  {/* Hover tooltip */}
                  {hoveredNode && (
                        <div className="absolute top-4 left-4 bg-black/80 backdrop-blur-sm border border-medium rounded-lg px-3 py-2 pointer-events-none">
                              <div className="flex items-center gap-2">
                                    <div
                                          className="w-2 h-2 rounded-full"
                                          style={{
                                                backgroundColor: hoveredNode.type === 'game' ? '#6366f1' :
                                                      hoveredNode.type === 'creator' ? '#22c55e' : '#f59e0b'
                                          }}
                                    />
                                    <span className="text-white text-sm font-medium">{hoveredNode.label}</span>
                              </div>
                              <span className="text-white/50 text-xs capitalize">{hoveredNode.type}</span>
                        </div>
                  )}

                  {/* Legend */}
                  <div className="absolute bottom-4 right-4 bg-black/60 backdrop-blur-sm border border-medium rounded-lg px-3 py-2 pointer-events-none">
                        <div className="flex flex-col gap-1.5 text-xs">
                              <div className="flex items-center gap-2">
                                    <div className="w-2 h-2 rounded-full bg-[#6366f1]" />
                                    <span className="text-white/70">Games</span>
                              </div>
                              <div className="flex items-center gap-2">
                                    <div className="w-2 h-2 rounded-full bg-[#22c55e]" />
                                    <span className="text-white/70">Creators</span>
                              </div>
                              <div className="flex items-center gap-2">
                                    <div className="w-2 h-2 rounded-full bg-[#f59e0b]" />
                                    <span className="text-white/70">Developers</span>
                              </div>
                        </div>
                  </div>
            </div>
      );
}

export default DiscoveryLens;
