module View (component) where

import Data.Maybe (Maybe (..))
import Effect.Aff.Class (class MonadAff)
import Halogen as H
import Halogen.HTML as HH
import Halogen.HTML.Events as HE
import Halogen.HTML.Properties as HP
import Prelude

import Model (Album (..))
import Model as Model

type State =
  { isLoaded :: Boolean
  , albums :: Array Album
  }

data Action
  = BeginLoad
  | LoadAlbum

component :: forall f i o m. MonadAff m => H.Component HH.HTML f i o m
component =
  H.mkComponent
    { initialState
    , render
    , eval: H.mkEval $ H.defaultEval
      { handleAction = handleAction
      , initialize = Just BeginLoad
      }
    }

initialState :: forall i. i -> State
initialState = const
  { isLoaded: false
  , albums: []
  }

render :: forall m. State -> H.ComponentHTML Action () m
render state =
  if not state.isLoaded
    then
      HH.p_ [ HH.text "Loading albums ..." ]
    else
      HH.div_
        [ HH.ul
          [ HP.id_ "album-list" ]
          (map renderAlbum state.albums)
        ]

renderAlbum :: forall m. Album -> H.ComponentHTML Action () m
renderAlbum (Album album) =
  HH.li
    [ HE.onClick \_ -> Just LoadAlbum ]
    [ HH.img
      [ HP.src $ "http://localhost:8233/thumb/" <> album.id
      , HP.alt $ album.title <> " by " <> album.artist
      ]
    , HH.strong_ [ HH.text album.title ]
    , HH.text " "
    , HH.span_ [ HH.text album.artist ]
    ]

handleAction :: forall o m. MonadAff m => Action -> H.HalogenM State Action () o m Unit
handleAction = case _ of
  BeginLoad -> do
    albums <- H.liftAff Model.getAlbums
    H.modify_ $ _ { isLoaded = true, albums = albums }
  LoadAlbum -> pure unit
