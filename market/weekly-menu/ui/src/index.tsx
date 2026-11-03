import { useState } from 'react'
import { Badge, Button, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ShoppingCart, Utensils } from 'lucide-react'

interface Ingredient {
  name: string
  amount: number
  unit: string
}
interface Recipe {
  id: string
  name: string
  baseServings: number
  ingredients: Ingredient[]
}
interface ShoppingItem {
  name: string
  amount: number
  unit: string
  from: string[]
  asIs: boolean
}
interface ShoppingOut {
  items: ShoppingItem[]
  meals: number
}

const DAYS = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun']
const MEALS = ['breakfast', 'lunch', 'dinner']

export default function WeeklyMenu() {
  const { t, errorMessage } = usePlugin()
  const [state, setState] = usePluginState<{ recipes: Recipe[]; menu: (string | null)[]; servings: number[] }>(
    'menu',
    { recipes: [], menu: Array(21).fill(null), servings: Array(21).fill(2) },
  )
  const [recipeName, setRecipeName] = useState('')
  const [recipeIngredients, setRecipeIngredients] = useState('')
  const [recipeServings, setRecipeServings] = useState(2) // tf-allow: 默认值

  const addRecipe = () => {
    if (!recipeName.trim() || !recipeIngredients.trim()) return
    const ingredients = recipeIngredients
      .split(/[,，]/)
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => {
        const [name, amount, unit] = part.split(':').map((s) => s.trim())
        return { name, amount: Number(amount) || 1, unit: unit ?? '适量' }
      })
    const next = [...(state?.recipes ?? []), { id: `r-${Date.now()}`, name: recipeName.trim(), baseServings: recipeServings, ingredients }]
    setState({ recipes: next, menu: state?.menu ?? Array(21).fill(null), servings: state?.servings ?? Array(21).fill(2) })
    setRecipeName('')
    setRecipeIngredients('')
  }

  const menu = state?.menu ?? Array(21).fill(null)
  const servings = state?.servings ?? Array(21).fill(2)
  const recipes = state?.recipes ?? []

  const shoppingArgs = recipes.length > 0 ? { menu: menu.map((recipeId, index) => ({ recipeId, servings: servings[index] })), recipes } : null
  const shopping = useDebouncedCall<ShoppingOut>(shoppingArgs ? 'shopping' : null, shoppingArgs, [state])

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Utensils />} title={t('ui.recipes')} bodyClassName="space-y-2 p-3">
          <Input value={recipeName} onChange={(event) => setRecipeName(event.target.value)} placeholder={t('ui.recipe')} aria-label={t('ui.recipe')} />
          <Input value={recipeIngredients} onChange={(event) => setRecipeIngredients(event.target.value)} placeholder={t('ui.ingredients')} aria-label={t('ui.ingredients')} />
          <div className="flex items-center gap-2">
            <Button size="sm" variant="secondary" onClick={addRecipe}>
              {t('ui.addRecipe')}
            </Button>
            <span className="text-xs text-fg-muted">{t('ui.servings')}</span>
            <Input type="number" value={String(recipeServings)} onChange={(event) => setRecipeServings(Number(event.target.value))} className="w-16" aria-label={t('ui.servings')} />
          </div>
          {(state?.recipes ?? []).map((recipe) => (
            <div key={recipe.id} className="flex items-center gap-2 border-t border-border pt-1.5 text-xs">
              <span className="min-w-0 flex-1 truncate font-medium text-fg">{recipe.name}</span>
              <span className="text-fg-subtle">{recipe.ingredients.length}</span>
              <button
                type="button"
                aria-label={t('ui.delete')}
                onClick={() => setState({ recipes: recipes.filter((r) => r.id !== recipe.id), menu, servings })}
                className="text-fg-subtle outline-none hover:text-danger focus-visible:ring-2 focus-visible:ring-ring"
              >
                {'×'}
              </button>
            </div>
          ))}
        </Panel>
      </div>
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Utensils />} title={t('api.shopping.summary')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto p-3" extra={shopping.pending && <Spinner className="size-3.5" />}>
          {recipes.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('ui.empty')}</p>
          ) : (
            <>
              <div className="grid grid-cols-[auto_repeat(3,1fr)] gap-1">
                <span />
                {MEALS.map((meal) => (
                  <span key={meal} className="text-center text-[11px] text-fg-subtle">{t(`meals.${meal}`)}</span>
                ))}
                {DAYS.map((day) => (
                  <>
                    <span key={`${day}-label`} className="self-center text-[11px] text-fg-subtle">{t(`days.${day}`)}</span>
                    {MEALS.map((meal, mealIndex) => {
                      const slot = DAYS.indexOf(day) * 3 + MEALS.indexOf(meal)
                      void mealIndex
                      return (
                        <Select
                          key={`${day}-${meal}`}
                          size="sm"
                          value={menu[slot] ?? ''}
                          onValueChange={(value) => {
                            const nextMenu = [...menu]
                            nextMenu[slot] = value || null
                            setState({ recipes, menu: nextMenu, servings })
                          }}
                          options={[{ value: '', label: '—' }, ...recipes.map((recipe) => ({ value: recipe.id, label: recipe.name }))]}
                          aria-label={`${t(`days.${day}`)} ${t(`meals.${meal}`)}`}
                        />
                      )
                    })}
                  </>
                ))}
              </div>
              <div className="border-t border-border pt-2">
                <p className="mb-1 flex items-center gap-1.5 text-xs font-medium text-fg-muted">
                  <ShoppingCart className="size-3.5" />
                  {t('ui.shopping')}
                  {shopping.result && ` · ${t('ui.meals')}: ${shopping.result.meals}`}
                </p>
                {shopping.error && <p className="text-xs text-danger">{errorMessage(shopping.error)}</p>}
                {(shopping.result?.items ?? []).map((item) => (
                  <div key={item.name} className="flex items-center justify-between py-0.5 text-xs">
                    <span className="font-medium text-fg">{item.name}</span>
                    <span className="text-fg-muted tabular-nums">
                      {item.asIs ? item.unit : `${item.amount} ${item.unit}`}
                      {item.asIs && <Badge variant="neutral" className="ml-1">{t('ui.asIs')}</Badge>}
                    </span>
                  </div>
                ))}
              </div>
            </>
          )}
        </Panel>
      </div>
    </div>
  )
}
