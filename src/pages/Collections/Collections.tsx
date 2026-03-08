// Collections — persistent user collections + wishlist + random pick.

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { toAssetUrl } from '../../hooks/api';
import { useToast } from '../../components/Toast';
import './Collections.css';

interface Collection {
      id: string;
      name: string;
      description: string;
      is_smart: boolean;
      smart_rule: string | null;
      sort_order: number;
      created_at: string;
}

interface WishlistEntry {
      id: string;
      title: string;
      developer: string | null;
      priority: number;
      created_at: string;
}

interface RandomWork {
      id: string;
      title: string;
      cover_path: string | null;
      developer: string | null;
      rating: number | null;
}

interface CollectionWork {
      id: string;
      title: string;
      cover_path: string | null;
      developer: string | null;
      rating: number | null;
}

const PRIORITY_LABELS: Record<number, string> = { 0: '—', 1: '🔵 Low', 2: '🟡 Medium', 3: '🔴 High' };

export default function Collections() {
      const { showToast } = useToast();
      const navigate = useNavigate();
      const [tab, setTab] = useState<'collections' | 'wishlist'>('collections');
      const [collections, setCollections] = useState<Collection[]>([]);
      const [wishlist, setWishlist] = useState<WishlistEntry[]>([]);
      const [showCreate, setShowCreate] = useState(false);
      const [newName, setNewName] = useState('');
      const [newDesc, setNewDesc] = useState('');
      const [randomWork, setRandomWork] = useState<RandomWork | null>(null);
      const [selectedCollection, setSelectedCollection] = useState<Collection | null>(null);
      const [collectionWorks, setCollectionWorks] = useState<CollectionWork[]>([]);
      const [loadingCollectionWorks, setLoadingCollectionWorks] = useState(false);
      const [draggingWorkId, setDraggingWorkId] = useState<string | null>(null);

      const [wishTitle, setWishTitle] = useState('');
      const [wishDev, setWishDev] = useState('');
      const [wishPriority, setWishPriority] = useState(0);
      const [showWishForm, setShowWishForm] = useState(false);

      const [isSmart, setIsSmart] = useState(false);
      const [ruleOp, setRuleOp] = useState<'and' | 'or'>('and');
      const [conditions, setConditions] = useState<{ field: string; op: string; value: string }[]>([]);

      const FIELDS = ['developer', 'rating', 'library_status', 'enrichment_state', 'title', 'tags', 'vndb_id', 'dlsite_id'];
      const OPS = ['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'contains', 'starts', 'is_null', 'not_null'];

      useEffect(() => {
            loadCollections();
            loadWishlist();
      }, []);

      const loadCollections = useCallback(async () => {
            try {
                  const data = await invoke<Collection[]>('list_collections');
                  const nextCollections = Array.isArray(data) ? data : [];
                  setCollections(nextCollections);

                  if (selectedCollection) {
                        const nextSelected = nextCollections.find((collection) => collection.id === selectedCollection.id) || null;
                        setSelectedCollection(nextSelected);
                        if (nextSelected) {
                              await loadCollectionWorks(nextSelected);
                        } else {
                              setCollectionWorks([]);
                        }
                  }
            } catch {
                  setCollections([]);
            }
      }, [selectedCollection]);

      const loadWishlist = useCallback(async () => {
            try {
                  const data = await invoke<WishlistEntry[]>('list_wishlist');
                  setWishlist(Array.isArray(data) ? data : []);
            } catch {
                  setWishlist([]);
            }
      }, []);

      async function loadCollectionWorks(collection: Collection) {
            setLoadingCollectionWorks(true);
            try {
                  const command = collection.is_smart ? 'evaluate_smart_collection' : 'get_collection_works';
                  const data = await invoke<CollectionWork[]>(command, { collectionId: collection.id });
                  setCollectionWorks(Array.isArray(data) ? data : []);
            } catch {
                  setCollectionWorks([]);
            } finally {
                  setLoadingCollectionWorks(false);
            }
      }

      function addCondition() {
            setConditions([...conditions, { field: 'developer', op: 'eq', value: '' }]);
      }

      function removeCondition(idx: number) {
            setConditions(conditions.filter((_, index) => index !== idx));
      }

      function updateCondition(idx: number, key: string, value: string) {
            const updated = [...conditions];
            (updated[idx] as Record<string, string>)[key] = value;
            setConditions(updated);
      }

      async function handleCreate() {
            if (!newName.trim()) {
                  showToast('Name required', 'error');
                  return;
            }

            const smartRule = isSmart && conditions.length > 0
                  ? JSON.stringify({ operator: ruleOp, conditions })
                  : null;

            try {
                  await invoke('create_collection', {
                        name: newName.trim(),
                        description: newDesc.trim(),
                        isSmart,
                        smartRule,
                  });
                  setNewName('');
                  setNewDesc('');
                  setShowCreate(false);
                  setIsSmart(false);
                  setConditions([]);
                  showToast(`Collection "${newName}" created`, 'success');
                  await loadCollections();
            } catch {
                  showToast('Failed to create collection', 'error');
            }
      }

      async function handleDelete(id: string, name: string) {
            try {
                  await invoke('delete_collection', { id });
                  if (selectedCollection?.id === id) {
                        setSelectedCollection(null);
                        setCollectionWorks([]);
                  }
                  showToast(`Deleted "${name}"`, 'info');
                  await loadCollections();
            } catch {
                  showToast('Delete failed', 'error');
            }
      }

      async function handleOpenCollection(collection: Collection) {
            setSelectedCollection(collection);
            await loadCollectionWorks(collection);
      }

      async function handleAddWish() {
            if (!wishTitle.trim()) {
                  showToast('Title required', 'error');
                  return;
            }
            try {
                  await invoke('add_wishlist', {
                        title: wishTitle.trim(),
                        developer: wishDev.trim() || null,
                        priority: wishPriority,
                  });
                  setWishTitle('');
                  setWishDev('');
                  setWishPriority(0);
                  setShowWishForm(false);
                  showToast('Added to wishlist', 'success');
                  await loadWishlist();
            } catch {
                  showToast('Failed to add', 'error');
            }
      }

      async function handleRemoveWish(id: string) {
            try {
                  await invoke('remove_wishlist', { id });
                  await loadWishlist();
            } catch {
                  showToast('Remove failed', 'error');
            }
      }

      async function handleRandomPick() {
            try {
                  const work = await invoke<RandomWork | null>('random_pick');
                  setRandomWork(work);
            } catch {
                  showToast('Random pick failed', 'error');
            }
      }

      async function handleReorder(targetWorkId: string) {
            if (!selectedCollection || selectedCollection.is_smart || !draggingWorkId || draggingWorkId === targetWorkId) {
                  return;
            }

            const fromIndex = collectionWorks.findIndex((work) => work.id === draggingWorkId);
            const toIndex = collectionWorks.findIndex((work) => work.id === targetWorkId);
            if (fromIndex < 0 || toIndex < 0) {
                  return;
            }

            const reordered = [...collectionWorks];
            const [moved] = reordered.splice(fromIndex, 1);
            reordered.splice(toIndex, 0, moved);
            setCollectionWorks(reordered);
            setDraggingWorkId(null);

            try {
                  await invoke('reorder_collection', {
                        collectionId: selectedCollection.id,
                        workIds: reordered.map((work) => work.id),
                  });
                  showToast('Collection order updated', 'success');
            } catch {
                  showToast('Failed to reorder collection', 'error');
                  await loadCollectionWorks(selectedCollection);
            }
      }

      return (
            <div className="collections-page">
                  <div className="collections-header">
                        <h1>Collections</h1>
                        <div className="coll-actions">
                              <button className="random-btn" onClick={handleRandomPick} title="Random pick">
                                    🎲 Random
                              </button>
                        </div>
                  </div>

                  {randomWork && (
                        <div className="random-result" onClick={() => navigate(`/work/${randomWork.id}`)}>
                              <span className="random-label">🎲 Your pick:</span>
                              <strong>{randomWork.title}</strong>
                              {randomWork.developer && <span className="random-dev"> — {randomWork.developer}</span>}
                              <button
                                    className="random-dismiss"
                                    onClick={(event) => {
                                          event.stopPropagation();
                                          setRandomWork(null);
                                    }}
                              >
                                    ✕
                              </button>
                        </div>
                  )}

                  <div className="coll-tabs">
                        <button className={tab === 'collections' ? 'active' : ''} onClick={() => setTab('collections')}>
                              📚 Collections ({collections.length})
                        </button>
                        <button className={tab === 'wishlist' ? 'active' : ''} onClick={() => setTab('wishlist')}>
                              💫 Wishlist ({wishlist.length})
                        </button>
                  </div>

                  {tab === 'collections' && (
                        <>
                              <button className="create-btn" onClick={() => setShowCreate(!showCreate)}>
                                    {showCreate ? '✕ Cancel' : '+ New Collection'}
                              </button>

                              {showCreate && (
                                    <div className="create-form stacked">
                                          <input type="text" placeholder="Collection name" value={newName} onChange={(e) => setNewName(e.target.value)} autoFocus />
                                          <input type="text" placeholder="Description (optional)" value={newDesc} onChange={(e) => setNewDesc(e.target.value)} />
                                          <label className="smart-toggle">
                                                <input type="checkbox" checked={isSmart} onChange={(e) => setIsSmart(e.target.checked)} />
                                                ⚡ Smart Collection
                                          </label>
                                          {isSmart && (
                                                <div className="rule-builder">
                                                      <div className="rule-header">
                                                            <select value={ruleOp} onChange={(e) => setRuleOp(e.target.value as 'and' | 'or')}>
                                                                  <option value="and">Match ALL (AND)</option>
                                                                  <option value="or">Match ANY (OR)</option>
                                                            </select>
                                                            <button className="add-condition-btn" onClick={addCondition}>+ Add Rule</button>
                                                      </div>
                                                      {conditions.map((condition, index) => (
                                                            <div key={index} className="condition-row">
                                                                  <select value={condition.field} onChange={(e) => updateCondition(index, 'field', e.target.value)}>
                                                                        {FIELDS.map((field) => <option key={field} value={field}>{field}</option>)}
                                                                  </select>
                                                                  <select value={condition.op} onChange={(e) => updateCondition(index, 'op', e.target.value)}>
                                                                        {OPS.map((op) => <option key={op} value={op}>{op}</option>)}
                                                                  </select>
                                                                  <input
                                                                        type="text"
                                                                        placeholder="value"
                                                                        value={condition.value}
                                                                        onChange={(e) => updateCondition(index, 'value', e.target.value)}
                                                                  />
                                                                  <button className="condition-remove" onClick={() => removeCondition(index)}>✕</button>
                                                            </div>
                                                      ))}
                                                </div>
                                          )}
                                          <button className="create-submit" onClick={handleCreate}>Create</button>
                                    </div>
                              )}

                              {collections.length === 0 ? (
                                    <div className="collections-empty">
                                          <span className="empty-icon">📚</span>
                                          <p>No collections yet</p>
                                          <p className="empty-hint">Create collections to organize your games</p>
                                    </div>
                              ) : (
                                    <>
                                          <div className="collections-grid">
                                                {collections.map((collection) => (
                                                      <article
                                                            key={collection.id}
                                                            className={`collection-card ${selectedCollection?.id === collection.id ? 'selected' : ''}`}
                                                            onClick={() => handleOpenCollection(collection)}
                                                      >
                                                            <div className="collection-icon">{collection.is_smart ? '⚡' : '📚'}</div>
                                                            <div className="collection-info">
                                                                  <h3>{collection.name}</h3>
                                                                  {collection.description && <p className="collection-desc">{collection.description}</p>}
                                                                  <span className="collection-type">{collection.is_smart ? 'Smart' : 'Manual'}</span>
                                                            </div>
                                                            <button
                                                                  className="collection-delete"
                                                                  onClick={(event) => {
                                                                        event.stopPropagation();
                                                                        handleDelete(collection.id, collection.name);
                                                                  }}
                                                                  title="Delete"
                                                            >
                                                                  🗑️
                                                            </button>
                                                      </article>
                                                ))}
                                          </div>

                                          {selectedCollection && (
                                                <section className="collection-panel">
                                                      <div className="collection-panel-header">
                                                            <div>
                                                                  <h2>{selectedCollection.name}</h2>
                                                                  {selectedCollection.description && <p>{selectedCollection.description}</p>}
                                                            </div>
                                                            <div className="collection-panel-meta">
                                                                  <span>{selectedCollection.is_smart ? 'Smart collection' : 'Manual collection'}</span>
                                                                  <span>{collectionWorks.length} works</span>
                                                            </div>
                                                      </div>

                                                      {selectedCollection.is_smart ? (
                                                            <p className="panel-hint">Smart collection results are read-only here. Update the rule to change membership.</p>
                                                      ) : (
                                                            <p className="panel-hint">Drag works to reorder them. The order is saved immediately.</p>
                                                      )}

                                                      {loadingCollectionWorks ? (
                                                            <div className="collection-works-empty">Loading works...</div>
                                                      ) : collectionWorks.length === 0 ? (
                                                            <div className="collection-works-empty">No works in this collection yet.</div>
                                                      ) : (
                                                            <div className="collection-works-list">
                                                                  {collectionWorks.map((work, index) => (
                                                                        <article
                                                                              key={work.id}
                                                                              className={`collection-work-row ${draggingWorkId === work.id ? 'dragging' : ''}`}
                                                                              draggable={!selectedCollection.is_smart}
                                                                              onDragStart={() => setDraggingWorkId(work.id)}
                                                                              onDragOver={(event) => event.preventDefault()}
                                                                              onDrop={() => handleReorder(work.id)}
                                                                        >
                                                                              <div className="collection-work-order">{index + 1}</div>
                                                                              <div className="collection-work-cover">
                                                                                    {work.cover_path ? (
                                                                                          <img src={toAssetUrl(work.cover_path) ?? ''} alt={work.title} />
                                                                                    ) : (
                                                                                          <span>🎮</span>
                                                                                    )}
                                                                              </div>
                                                                              <div className="collection-work-info" onClick={() => navigate(`/work/${work.id}`)}>
                                                                                    <h3>{work.title}</h3>
                                                                                    <p>{work.developer || 'Unknown developer'}</p>
                                                                              </div>
                                                                              <div className="collection-work-meta">
                                                                                    {work.rating !== null && <span>★ {work.rating.toFixed(1)}</span>}
                                                                                    {!selectedCollection.is_smart && <span className="drag-handle">↕ Drag</span>}
                                                                              </div>
                                                                        </article>
                                                                  ))}
                                                            </div>
                                                      )}
                                                </section>
                                          )}
                                    </>
                              )}
                        </>
                  )}

                  {tab === 'wishlist' && (
                        <>
                              <button className="create-btn" onClick={() => setShowWishForm(!showWishForm)}>
                                    {showWishForm ? '✕ Cancel' : '+ Add Wish'}
                              </button>

                              {showWishForm && (
                                    <div className="create-form">
                                          <input type="text" placeholder="Game title" value={wishTitle} onChange={(e) => setWishTitle(e.target.value)} autoFocus />
                                          <input type="text" placeholder="Developer (optional)" value={wishDev} onChange={(e) => setWishDev(e.target.value)} />
                                          <select value={wishPriority} onChange={(e) => setWishPriority(Number(e.target.value))}>
                                                <option value={0}>No priority</option>
                                                <option value={1}>🔵 Low</option>
                                                <option value={2}>🟡 Medium</option>
                                                <option value={3}>🔴 High</option>
                                          </select>
                                          <button className="create-submit" onClick={handleAddWish}>Add</button>
                                    </div>
                              )}

                              {wishlist.length === 0 ? (
                                    <div className="collections-empty">
                                          <span className="empty-icon">💫</span>
                                          <p>Wishlist empty</p>
                                          <p className="empty-hint">Add games you want to play later</p>
                                    </div>
                              ) : (
                                    <div className="wishlist-list">
                                          {wishlist.map((wish) => (
                                                <div key={wish.id} className="wish-row">
                                                      <span className="wish-priority">{PRIORITY_LABELS[wish.priority] || '—'}</span>
                                                      <span className="wish-title">{wish.title}</span>
                                                      {wish.developer && <span className="wish-dev">{wish.developer}</span>}
                                                      <button className="wish-remove" onClick={() => handleRemoveWish(wish.id)}>✕</button>
                                                </div>
                                          ))}
                                    </div>
                              )}
                        </>
                  )}
            </div>
      );
}
